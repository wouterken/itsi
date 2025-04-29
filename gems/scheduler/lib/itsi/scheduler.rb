# frozen_string_literal: true

require_relative "scheduler/version"
require_relative "scheduler/itsi_scheduler"
require_relative "schedule_refinement"

module Itsi
  class Scheduler
    class Error < StandardError; end

    def self.resume_token
      @resume_token ||= 0
      @resume_token += 1
    end

    def initialize
      @join_waiters = {}.compare_by_identity
      @token_map = {}.compare_by_identity
      @resume_tokens = {}.compare_by_identity
      @unblocked = [[], []]
      @unblock_idx = 0
      @unblocked_mux = Mutex.new
      @resume_fiber = method(:resume_fiber).to_proc
      @resume_fiber_with_readiness = method(:resume_fiber_with_readiness).to_proc
      @resume_blocked = method(:resume_blocked).to_proc
    end

    def block(_, timeout, fiber = Fiber.current, token = Scheduler.resume_token)
      @join_waiters[fiber] = true

      start_timer(timeout, token) if timeout
      @resume_tokens[token] = fiber
      @token_map[fiber] = token
      Fiber.yield
    ensure
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
      @closed ||= true
      freeze
    end

    # Need to defer to Process::Status rather than our extension
    # as we don't have a means of creating our own Process::Status.
    def process_wait(pid, flags)
      result = nil
      thread = Thread.new do
        result = Process::Status.wait(pid, flags)
      end
      thread.join
      result
    end

    def closed?
      @closed
    end

    # Spin up a new fiber and immediately resume it.
    def fiber(&blk)
      Fiber.new(blocking: false, &blk).tap(&:resume)
    end
  end
end
