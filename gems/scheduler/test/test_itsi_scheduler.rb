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
    assert_match /Failed to resume fiber /, out
  end
end
