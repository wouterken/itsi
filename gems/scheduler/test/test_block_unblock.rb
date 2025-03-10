# frozen_string_literal: true


class TestBlockUnblock < Minitest::Test
  include Itsi::Scheduler::TestHelper

  def test_block_with_timeout
    result = nil
    start_time = Time.now

    with_scheduler do |_scheduler|
      Fiber.schedule do
        # Simulate a blocking operation with a 0.1-second timeout.
        Fiber.scheduler.block(nil, 0.1)
        result = :resumed
      end
    end

    elapsed_time = Time.now - start_time

    assert_equal :resumed, result
    assert_in_delta 0.1, elapsed_time, 0.01, "Fiber did not resume after the expected timeout"
  end

  # Test that a fiber blocked without a timeout can be manually unblocked.
  def test_block_and_manual_unblock
    result = nil

    with_scheduler do |scheduler|
      fiber = Fiber.schedule do
        # This fiber blocks indefinitely until manually unblocked.
        Fiber.scheduler.block(:self, nil)
        result = :resumed
      end

      Fiber.schedule do
        sleep 0.05
        scheduler.unblock(:self, fiber)
      end
    end

    assert_equal :resumed, result
  end

  def test_double_unblock
    result = nil

    fiber = scheduler = nil



    Thread.new do
      sleep 0.01
      scheduler.unblock(:self, fiber)
      scheduler.unblock(:self, fiber)
    end

    with_scheduler do |sched|
      scheduler = sched
      fiber = Fiber.schedule do
        # This fiber blocks indefinitely until manually unblocked.
        sleep 0.1
        Fiber.scheduler.block(:self, nil)
        result = :resumed
      end
    end


    assert_equal :resumed, result
  end

  def test_timed_double_unblock
    result = nil

    fiber = scheduler = nil

    Thread.new do
      sleep 0.01
      scheduler.unblock(:self, fiber)
      scheduler.unblock(:self, fiber)
      sleep 1
    end

    with_scheduler do |sched|
      scheduler = sched
      fiber = Fiber.schedule do
        # This fiber blocks indefinitely until manually unblocked.
        Fiber.scheduler.block(:self, 0.1)
        Fiber.scheduler.block(:self, 0.1)
        result = :resumed
      end
    end

    assert_equal :resumed, result
  end

  def test_condition_variable_signaling
    result = nil
    mutex = Mutex.new
    cv = ConditionVariable.new

    # Set a scheduler so that non-blocking fibers are active.
    # (Without a scheduler, blocking operations run in the usual way.)
    with_scheduler do
      # Create a non-blocking fiber that waits on the condition variable.
      fiber = Fiber.schedule do
        mutex.synchronize do
          # The call to cv.wait(mutex) internally triggers the scheduler’s block hook.
          cv.wait(mutex)
          result = :resumed
        end
      end

      # In a separate thread, signal the condition variable after a delay.
      Thread.new do
        sleep 0.05
        mutex.synchronize do
          cv.signal # This should cause Ruby to internally call the scheduler’s unblock hook.
        end
      end

      # Wait until the fiber finishes.
      Timeout.timeout(1) do
        sleep 0.01 while fiber.alive?
      end
    end
    assert_equal :resumed, result
  end

  def test_queue_pop_unblocks_fiber
    result = nil
    queue = Queue.new

    # Set a scheduler so that non-blocking fiber operations are enabled.
    with_scheduler do
      # Schedule a fiber that waits on the queue.
      fiber = Fiber.schedule do
        # queue.pop will block until an item is pushed.
        result = queue.pop
      end

      # In a separate thread, push an element into the queue after a short delay.
      Thread.new do
        queue.push(:hello)
      end.join

      # Wait until the fiber finishes execution.
      Timeout.timeout(1) do
        sleep 0.001 while fiber.alive?
      end
    end
    assert_equal :hello, result
  end

  # Test that unblocking a fiber that isn’t blocked is a no-op.
  def test_unblock_non_blocked_fiber
    with_scheduler do |scheduler|
      fiber = Fiber.new do
        # Do nothing special.
        :finished
      end

      # Try to unblock a fiber that isn’t currently blocked.
      scheduler.unblock(nil, fiber)
      # The fiber should finish normally.
      assert_equal :finished, fiber.resume
    end
  end

  def test_multiple_fibers_blocking_and_unblocking
    results = {}

    with_scheduler do |scheduler|
      Fiber.schedule do
        Fiber.scheduler.block(:resource1, 0.1)
        results[:fiber1] = :resumed
      end

      fiber2 = Fiber.schedule do
        Fiber.scheduler.block(:resource2, nil)
        results[:fiber2] = :resumed
      end

      Fiber.schedule do
        sleep 0.05
        scheduler.unblock(:resource, fiber2)
      end
    end

    assert_equal :resumed, results[:fiber1], "Fiber1 did not resume after timeout"
    assert_equal :resumed, results[:fiber2], "Fiber2 did not resume after manual unblock"
  end

  def test_block_with_immediate_unblock
    result = nil

    with_scheduler do |scheduler|
      fiber = Fiber.schedule do
        Fiber.scheduler.block(:resource, 0.1)
        result = :resumed
      end

      Fiber.schedule do
        scheduler.unblock(:resource, fiber)
      end
    end

    assert_equal :resumed, result
  end

  def test_block_with_no_timeout_and_no_unblock
    result = nil

    with_scheduler(join: false) do |_scheduler|
      Fiber.schedule do
        Fiber.scheduler.block(:resource, nil)
        result = :resumed
      end

      # No unblock is called, and no timeout is set.
      sleep 0.1
    end

    sleep 0.2

    assert_nil result, "Fiber should remain blocked indefinitely"
  end

  def test_unblock_non_blocked_fiber_v2
    result = :not_resumed

    with_scheduler do |scheduler|
      fiber = Fiber.schedule do
        result = :resumed
      end

      Fiber.schedule do
        sleep 0.05
        scheduler.unblock(:resource, fiber)
      end
    end

    assert_equal :resumed, result, "Fiber should have been resumed normally"
  end
end
