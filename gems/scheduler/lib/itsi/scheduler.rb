# frozen_string_literal: true

require "etc"

require_relative "scheduler/version"
require_relative "scheduler/native_extension"
require_relative "schedule_refinement"

module Itsi
  class Scheduler
    class Error < StandardError; end
    WorkRequest = Struct.new(:fiber, :work, :result, :error, keyword_init: true)

    def self.resume_token
      @resume_token ||= 0
      @resume_token += 1
    end

    def initialize
      @join_waiters = {}.compare_by_identity
      @token_map = {}.compare_by_identity
      @resume_tokens = {}.compare_by_identity
      @timeout_requests = {}
      @unblocked = [[], []]
      @unblock_idx = 0
      @unblocked_mux = Mutex.new
      @resume_fiber = method(:resume_fiber).to_proc
      @resume_fiber_with_readiness = method(:resume_fiber_with_readiness).to_proc
      @resume_blocked = method(:resume_blocked).to_proc
      setup_worker_pool
    end

    def block(_, timeout, fiber = Fiber.current, token = Scheduler.resume_token)
      @join_waiters[fiber] = true

      start_timer(timeout, token) if timeout
      @resume_tokens[token] = fiber
      @token_map[fiber] = token
      Fiber.yield
    ensure
      cancel_wait(token)
      @resume_tokens.delete(token)
      @token_map.delete(fiber)
      @join_waiters.delete(fiber)
    end

    # Register an IO waiter.
    # This will get resumed by our scheduler inside the call to
    # fetch_events.
    def io_wait(io, events, duration)
      fiber = Fiber.current
      token = Scheduler.resume_token
      readiness = register_io_wait(io.fileno, events, duration, token)
      readiness ||= block(nil, duration, fiber, token)
      clear_timer(token)
      readiness
    end

    def unblock(_blocker, fiber)
      @unblocked_mux.synchronize do
        @unblocked[@unblock_idx] << fiber
      end
      wake
    end

    def kernel_sleep(duration)
      block nil, duration
    end

    def timeout_after(duration, klass = Timeout::Error, message = "execution expired")
      fiber = Fiber.current
      token = Scheduler.resume_token
      exception = klass.is_a?(Class) ? klass.new(message) : klass
      @timeout_requests[token] = [fiber, exception]
      start_timer(duration, token)
      yield duration
    ensure
      clear_timer(token) if token
      @timeout_requests.delete(token) if token
    end

    def fiber_interrupt(fiber, exception)
      cancel_wait(@token_map[fiber]) if @token_map.key?(fiber)
      fiber.raise(exception)
      true
    rescue FiberError
      false
    end

    def blocking_operation_wait(work)
      request = WorkRequest.new(fiber: Fiber.current, work: work)
      @worker_queue << request
      block(nil, nil, request.fiber)
      raise request.error if request.error

      request.result
    end

    def io_select(readables, writables, exceptables, timeout)
      readables = Array(readables).compact
      writables = Array(writables).compact
      exceptables = Array(exceptables).compact
      ios = (readables + writables + exceptables).uniq

      if ios.length == 1
        io = ios.first
        events = 0
        events |= IO::READABLE if readables.include?(io)
        events |= IO::WRITABLE if writables.include?(io)
        events |= IO::PRIORITY if exceptables.include?(io)
        readiness = io_wait(io, events, timeout)
        return nil unless readiness

        return [
          (readiness & IO::READABLE).zero? ? [] : readables.select { |entry| entry == io },
          (readiness & IO::WRITABLE).zero? ? [] : writables.select { |entry| entry == io },
          (readiness & IO::PRIORITY).zero? ? [] : exceptables.select { |entry| entry == io }
        ]
      end

      blocking_operation_wait(-> { IO.select(readables, writables, exceptables, timeout) })
    end

    def tick
      events = fetch_due_events
      timers = fetch_due_timers
      unblocked = switch_unblock_batch
      events&.each(&@resume_fiber_with_readiness)
      unblocked.each(&@resume_blocked)
      unblocked.clear
      timers&.each(&@resume_fiber)
    end

    def resume_fiber(token)
      if (request = @timeout_requests.delete(token))
        fiber, exception = request
        fiber_interrupt(fiber, exception)
        return
      end

      if (fiber = @resume_tokens.delete(token))
        fiber.resume
      end
    rescue StandardError => e
      warn "Fiber #{fiber} terminated on exception: #{e.message}"
    end

    def resume_fiber_with_readiness((token, readiness))
      if (fiber = @resume_tokens.delete(token))
        fiber.resume(readiness)
      end
    rescue StandardError => e
      warn "Fiber #{fiber} terminated on exception: #{e.message}"
    end

    def resume_blocked(fiber)
      if (token = @token_map[fiber])
        resume_fiber(token)
      elsif fiber.alive?
        fiber.resume
      end
    end

    def switch_unblock_batch
      @unblocked_mux.synchronize do
        current = @unblocked[@unblock_idx]
        @unblock_idx = (@unblock_idx + 1) % 2
        current
      end
    end

    # Yields upwards to the scheduler, with an intention to
    # resume the fiber that yielded ASAP.
    def yield
      kernel_sleep(0) if work?
    end

    # Keep running until we've got no timers we're awaiting, no pending IO, no temporary yields,
    # no pending unblocks.
    def work?
      !@unblocked[@unblock_idx].empty? || !@join_waiters.empty? || has_pending_io?
    end

    # Run until no more work needs doing.
    def run
      tick while work?
      debug "Exit Scheduler"
    end

    # Hook invoked at the end of the thread.
    # Will start our scheduler's Reactor.
    def close
      run
    ensure
      shutdown_worker_pool
      @closed ||= true
      freeze
    end

    # Need to defer to Process::Status rather than our extension
    # as we don't have a means of creating our own Process::Status.
    def process_wait(pid, flags)
      blocking_operation_wait(-> { Process::Status.wait(pid, flags) })
    end

    def address_resolve(hostname)
      blocking_operation_wait(-> { native_address_resolve(hostname) })
    end

    def process_fork
      shutdown_worker_pool
      setup_worker_pool
      nil
    end

    def closed?
      @closed
    end

    # Spin up a new fiber and immediately resume it.
    def fiber(&blk)
      Fiber.new(blocking: false, &blk).tap(&:resume)
    end

    private

    def setup_worker_pool
      @worker_stop_token = Object.new
      @worker_queue = Queue.new
      @worker_threads = Array.new(worker_pool_size) { start_worker_thread }
    end

    def start_worker_thread
      Thread.new do
        Thread.current.report_on_exception = false
        Thread.current.thread_variable_set(:fork_safe, true)

        loop do
          request = @worker_queue.pop
          break if request.equal?(@worker_stop_token)

          begin
            request.result = request.work.call
          rescue Exception => exception
            request.error = exception
          ensure
            unblock(nil, request.fiber)
          end
        end
      end
    end

    def shutdown_worker_pool
      return unless @worker_threads

      @worker_threads.size.times { @worker_queue << @worker_stop_token }
      @worker_threads.each(&:join)
      @worker_threads.clear
    end

    def worker_pool_size
      size = ENV.fetch("ITSI_WORKER_POOL_SIZE", Etc.nprocessors.to_s).to_i
      size.positive? ? size : 1
    rescue StandardError
      1
    end
  end
end
