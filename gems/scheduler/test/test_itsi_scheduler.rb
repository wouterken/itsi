# frozen_string_literal: true


class TestItsiScheduler < Minitest::Test
  include Itsi::Scheduler::TestHelper

  def test_that_it_has_a_version_number
    refute_nil ::Itsi::Scheduler::VERSION
  end

  def test_errors
    results = []
    start_at = Time.now
    # Run the scheduler in a dedicated thread to avoid interference with the
    # main thread’s scheduler state.
    total = 0
    out, err = capture_subprocess_io do
      with_scheduler do |_scheduler|
        9.times do |i|
          Fiber.schedule do
            sleep 0.05
            raise i if i % 3 == 0
            total += 1
          end
        end
      end
    end

    assert_equal total, 6
    assert_match /terminated on exception/, out
  end

  def test_blocking_operation_wait_returns_result_without_stalling_scheduler
    result = nil
    marker = nil

    with_scheduler do |scheduler|
      Fiber.schedule do
        result = scheduler.blocking_operation_wait(-> do
          sleep 0.05
          :done
        end)
      end

      Fiber.schedule do
        sleep 0.01
        marker = :progressed
      end
    end

    assert_equal :done, result
    assert_equal :progressed, marker
  end

  def test_blocking_operation_wait_propagates_exceptions
    error = nil

    with_scheduler do |scheduler|
      Fiber.schedule do
        begin
          scheduler.blocking_operation_wait(-> { raise ArgumentError, "boom" })
        rescue => exception
          error = exception
        end
      end
    end

    refute_nil error
    assert_equal ArgumentError, error.class
    assert_equal "boom", error.message
  end

  def test_io_select_returns_ready_descriptors_without_stalling_scheduler
    ready = nil
    marker = nil
    reader, writer = IO.pipe

    with_scheduler do |scheduler|
      Fiber.schedule do
        ready = scheduler.io_select([reader], nil, nil, 0.2)
      end

      Fiber.schedule do
        sleep 0.01
        marker = :progressed
        writer.write("x")
      end
    end

    assert_equal :progressed, marker
    assert_equal [[reader], [], []], ready
  ensure
    reader&.close
    writer&.close
  end

  def test_process_fork_reinitializes_worker_pool
    worker_threads = nil
    refreshed_threads = nil

    with_scheduler do |scheduler|
      worker_threads = scheduler.instance_variable_get(:@worker_threads)
      scheduler.process_fork
      refreshed_threads = scheduler.instance_variable_get(:@worker_threads)
    end

    refute_nil worker_threads
    refute_nil refreshed_threads
    refute_same worker_threads, refreshed_threads
    assert_equal worker_threads.length, refreshed_threads.length
  end
end
