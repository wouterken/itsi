# frozen_string_literal: true


class TestProcessWait < Minitest::Test
  include Itsi::Scheduler::TestHelper

  def test_process_wait
    start_time = Time.now
    pids = []

    with_scheduler do |_scheduler|
      3.times do
        pids << Process.spawn("sleep 0.25")
      end
      3.times do |i|
        Fiber.schedule do
          Process.wait(pids[i])
        end
      end
    end
    end_time = Time.now
    assert_in_delta(0.25, end_time - start_time, 0.1)

  end
end
