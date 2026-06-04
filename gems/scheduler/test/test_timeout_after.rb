# frozen_string_literal: true

require "timeout"

class TestTimeoutAfter < Minitest::Test
  include Itsi::Scheduler::TestHelper

  def test_timeout_after_raises_timeout_error
    result = nil

    with_scheduler do
      Fiber.schedule do
        begin
          Timeout.timeout(0.01, Timeout::Error, "benchmark timeout") do
            sleep 0.05
          end
        rescue Timeout::Error => error
          result = error.message
        end
      end
    end

    assert_equal "benchmark timeout", result
  end

  def test_timeout_after_cancels_fast_work_without_waiting_for_timeout
    result = nil
    started_at = Process.clock_gettime(Process::CLOCK_MONOTONIC)

    with_scheduler do
      Fiber.schedule do
        Timeout.timeout(0.05, Timeout::Error, "benchmark timeout") do
          sleep 0.001
          result = :completed
        end
      end
    end

    elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - started_at

    assert_equal :completed, result
    assert_operator elapsed, :<, 0.03
  end

  def test_timeout_after_interrupts_io_wait_without_leaking_waiters
    result = nil
    reader, writer = IO.pipe
    started_at = Process.clock_gettime(Process::CLOCK_MONOTONIC)

    with_scheduler do
      Fiber.schedule do
        begin
          Timeout.timeout(0.01, Timeout::Error, "benchmark timeout") do
            reader.readpartial(1)
          end
        rescue Timeout::Error
          result = :timed_out
        end
      end
    end

    elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - started_at

    assert_equal :timed_out, result
    assert_operator elapsed, :<, 0.05
  ensure
    reader&.close
    writer&.close
  end

  def test_cascading_timeout_workload_completes
    completed = 0
    timed_out = 0
    total = 0

    with_scheduler do
      20.times do |index|
        Fiber.schedule do
          50.times do |iteration|
            slow = ((iteration * 7 + index * 3) % 10) < 3

            begin
              Timeout.timeout(0.01, Timeout::Error, "benchmark timeout") do
                sleep(slow ? 0.05 : 0.002)
                completed += 1
              end
            rescue Timeout::Error
              timed_out += 1
            ensure
              total += 1
            end
          end
        end
      end
    end

    assert_equal 1000, total
    assert_operator completed, :>, 0
    assert_operator timed_out, :>, 0
    assert_equal total, completed + timed_out
  end
end
