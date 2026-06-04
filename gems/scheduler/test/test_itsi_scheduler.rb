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
end
