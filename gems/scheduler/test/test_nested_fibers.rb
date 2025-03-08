# frozen_string_literal: true

require_relative "test_helper"
require "active_record"

class TestNestedFibers < Minitest::Test
  include Itsi::Scheduler::TestHelper

  def each_pop(queue)
    loop do
      item = queue.pop
      break unless item

      yield item
    end
  end

  def test_sse_equivalent
    results = []
    queue = Queue.new
    request = nil
    sched_thread = with_scheduler(join: false) do
      request = Fiber.schedule do
        enumerator = to_enum(:each_pop, queue)
        loop do
          nxt = enumerator.next
          sleep nxt * 0.001
          results << nxt
        rescue StopIteration
          break
        end
      end

      Thread.new do
        10.times do |i|
          queue.push(i)
          sleep 0.1
        end
        queue.push(nil)
      end.join
    end.join

    assert_equal [*0...10], results
  end

  def test_base_unowned_no_scheduler_transfer
    results = []
    scheduler = nil
    outer = Fiber.new do
      inner = Fiber.new do
        results << 0
        sleep 0.02
        scheduler.transfer
        results << 4
      end

      results << 1
      inner.transfer
      results << 2
      inner.transfer
    end

    scheduler = Fiber.new do
      outer.transfer while outer.alive?
    end
    scheduler.transfer

    assert_equal [1, 0, 2, 4], results
  end

  def test_base_unowned_no_scheduler_yield
    results = []
    scheduler = nil
    outer = Fiber.new do
      inner = Fiber.new do
        results << 0
        Fiber.yield
        results << 4
      end

      results << 1
      inner.resume
      results << 2
      inner.resume
    end.resume

    assert_equal [1, 0, 2, 4], results
  end

  def test_base_unowned_no_scheduler_combine_resume_and_transfer
    results = []
    scheduler = nil
    inner = nil
    outer = Fiber.new do
      inner = Fiber.new do
        results << 0
        scheduler.transfer(inner)
        results << 4
      end
      results << 1
      inner.resume
      results << 2
    end
    scheduler = Fiber.new do |fib|
      fib = fib.transfer while fib
    end
    scheduler.transfer(outer)

    assert_equal [1, 0, 4, 2], results
  end

  def test_base_unowned_no_scheduler_combine_resume_and_transfer_with_scheduler
    results = []
    with_scheduler do |scheduler|
      results = []
      inner = nil
      outer = Fiber.new do
        inner = Fiber.new do
          results << 0
          sleep 2
          results << 4
        end
        results << 1
        inner.resume
        results << 2
      end
      outer.transfer
    end

    assert_equal [1, 0, 4, 2], results
  end

  def test_foo_bar

    fiber_2 = fiber_4 = fiber_1 = nil

    fiber_4 = Fiber.new do
      puts "Fiber 4 goes straight back to 3"
    end

    fiber_3 = Fiber.new do
      puts "Fiber 3 hands control back to 2"
      fiber_4.transfer
      puts "Now shifting back to 2"
      fiber_2.transfer
      puts "Ends up in 3"
    end

    fiber_2 = Fiber.new do
      puts "Fiber 2 hands control to 3"
      fiber_1.transfer
      "Fiber 2 explicit yield. Do I end up in 3 or 1?"
    end

    fiber_1 = Fiber.new do
      puts "Fiber 1 hands control to 2"
      fiber_2.transfer
      puts "Ends up in 1"
    end

    fiber_1.resume
    puts "Done it all"
  end

  def test_base_owned_with_scheduler
    results = []
    with_scheduler do |scheduler|
      scheduler = nil

      Fiber.schedule do
        Fiber.schedule do
          inner = Fiber.schedule do
            results << 0
            sleep 0.01
            results << 4
          end

          results << 1
          sleep 0.03
          results << 2
          sleep 0.01
        end
      end
    end
    assert_equal [0, 1, 4, 2], results
  end

  def test_nested_owned_fibers
    results = []
    with_scheduler do |scheduler|
      outer = Fiber.schedule do
        Fiber.schedule do
          results << 0
          sleep 0.02
          results << 4
        end

        Fiber.schedule do
          results << 1
          sleep 0.2
          Fiber.schedule do
            results << 5
            sleep 0.2
            results << 7
          end
          results << 6
        end
        results << 2
        sleep 0.01
        results << 3
      end
    end
    assert_equal [0, 1, 2, 3, 4, 5, 6, 7], results
  end

  def test_nested_unowned_fibers
    results = []
    with_scheduler do |scheduler|
      Fiber.new do
        fib = Fiber.new do
          results << 4
          sleep 0.001
          results << 5
          sleep 0.001
          results << 6
        end

        Fiber.new do
          results << 0
          sleep 0.001
          Fiber.new do
            results << 1
            sleep 0.1
            results << 8
          end.transfer
          results << 2
        end.resume

        results << 3

        fib.resume
        sleep 0.01
        results << 7
      end.transfer
    end
    assert_equal [0, 1, 8], results
  end

  def test_nested_unowned_fibers_no_scheduler
    results = []
    Fiber.new do
      fib = Fiber.new do
        results << 5
        sleep 0.001
        results << 6
        sleep 0.001
        results << 7
      end

      Fiber.new do
        results << 0
        sleep 0.001
        Fiber.new do
          results << 1
          sleep 0.1
          results << 2
        end.resume
        results << 3
      end.resume

      results << 4
      fib.resume
      sleep 0.01
      results << 8
    end.transfer

    assert_equal [0, 1, 2, 3, 4, 5, 6, 7, 8], results
  end

  def test_nested_owned_fibers
    results = []
    with_scheduler do |scheduler|
      Fiber.schedule do
        fib = Fiber.new do
          results << 2
          sleep 0.001
          results << 5
          sleep 0.001
          results << 6
        end

        Fiber.schedule do
          results << 0
          sleep 0.001
          Fiber.schedule do
            results << 3
            sleep 0.1
            results << 8
          end
          results << 4
        end

        results << 1
        fib.resume
        sleep 0.01
        results << 7
      end
    end
    assert_equal [0, 1, 2, 3, 4, 5, 6, 7, 8], results
  end
end
