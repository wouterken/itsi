# frozen_string_literal: true

require_relative "test_helper"

class TestKernelSleep < Minitest::Test
  include Itsi::Scheduler::TestHelper

  def test_that_it_has_a_version_number
    refute_nil ::Itsi::Scheduler::VERSION
  end

  def test_it_can_sleep_concurrently
    results = []
    start_at = Time.now
    # Run the scheduler in a dedicated thread to avoid interference with the
    # main thread’s scheduler state.
    with_scheduler do |_scheduler|
      5.times do
        Fiber.schedule do
          sleep 0.05
          results << "first"
          sleep 0.05
          results << "second"
        end
      end
    end

    ends_at = Time.now
    # We expect 10 sleep completions overall (2 per fiber).
    assert_equal 10, results.size
    # Because all sleeps run concurrently, the total elapsed time should be about 1 second.
    assert_in_delta 0.1, ends_at - start_at, 0.02, "Total elapsed time should be close to 0.1 second"
  end

  def test_sleep_zero_duration
    with_scheduler do |_scheduler|
      start = Time.now
      result = sleep(0)
      finish = Time.now
      assert result, "sleep(0) should return a truthy value"
      # Expect near-immediate return.
      assert_operator finish - start, :<, 0.01, "sleep(0) should not delay execution"
    end
  end

  def test_multiple_sleeps_in_single_fiber
    order = []
    with_scheduler do |_scheduler|
      # Schedule one fiber that sleeps twice.
      Fiber.schedule do
        order << "start"
        sleep 0.2
        order << "after first sleep"
        sleep 0.3
        order << "after second sleep"
      end
    end

    assert_equal ["start", "after first sleep", "after second sleep"], order
  end

  def test_fibers_wake_in_correct_order
    order = []
    with_scheduler do |_scheduler|
      # Schedule three fibers with different sleep durations.
      Fiber.schedule do
        sleep 0.3
        order << "fiber1"
      end
      Fiber.schedule do
        sleep 0.1
        order << "fiber2"
      end
      Fiber.schedule do
        sleep 0.2
        order << "fiber3"
      end
    end

    # Since the fibers sleep for 0.1, 0.2, and 0.3 seconds respectively,
    # we expect the wake-up order to be: fiber2, then fiber3, then fiber1.
    assert_equal %w[fiber2 fiber3 fiber1], order
  end

  def test_invalid_sleep_value
    with_scheduler do |_scheduler|
      assert_raises(TypeError) do
        sleep("not a number")
      end
    end
  end
end
