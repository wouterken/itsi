# frozen_string_literal: true


class TestFileIO < Minitest::Test
  include Itsi::Scheduler::TestHelper

  # Test that a fiber waiting to read 5 bytes from a pipe is resumed
  # when another fiber writes "hello" to the pipe.
  def test_io_read_resume
    reader, writer = IO.pipe
    result = nil

    with_scheduler do |_scheduler|
      Fiber.schedule do
        result = reader.read(5)
      end

      Fiber.schedule do
        sleep 0.05
        writer.write("hello")
      end
    end

    reader.close
    writer.close

    assert_equal "hello", result
  end

  # Test that IO.wait_readable times out correctly when no data arrives.
  def test_io_wait_readable_timeout
    reader, writer = IO.pipe

    result = nil
    with_scheduler do |_scheduler|
      Fiber.schedule do
        # When no data is available, wait_readable should return nil after the timeout.
        result = reader.wait_readable(0.05)
      end
    end

    reader.close
    writer.close

    assert_nil result, "Expected nil on timeout when no data is available"
  end

  # Test that two different pipes can be read concurrently.
  def test_multiple_io_reads
    reader1, writer1 = IO.pipe
    reader2, writer2 = IO.pipe
    results = {}

    with_scheduler do |_scheduler|
      Fiber.schedule do
        results[:first] = reader1.read(5)
      end

      Fiber.schedule do
        results[:second] = reader2.read(6)
      end

      Fiber.schedule do
        sleep 0.05
        writer1.write("first")
        writer2.write("second")
      end
    end

    reader1.close
    writer1.close
    reader2.close
    writer2.close

    assert_equal "first", results[:first]
    assert_equal "second", results[:second]
  end

  # Test an interleaved read/write operation.
  # One fiber writes the data in chunks (with short sleeps in between),
  # while another fiber reads in fixed-size chunks until EOF.
  def test_io_interleaved_read_write
    reader, writer = IO.pipe
    data_read = "".dup

    with_scheduler do |_scheduler|
      Fiber.schedule do
        loop do
          chunk = reader.read(3)
          break if chunk.nil? || chunk.empty?

          data_read << chunk
        end
      end

      Fiber.schedule do
        ["Hel", "lo ", "Wor", "ld"].each do |part|
          writer.write(part)
          sleep 0.02
        end
        writer.close # signal EOF to the reader
      end
    end

    reader.close

    assert_equal "Hello World", data_read
  end

  # Test that a fiber using nonblocking I/O correctly waits for data.
  def test_io_read_with_nonblocking_mode
    reader, writer = IO.pipe
    # Ensure the IOs are in synchronous mode.
    reader.sync = true
    writer.sync = true
    result = nil

    with_scheduler do |_scheduler|
      Fiber.schedule do
        # Use wait_readable to wait until data is available.
        reader.wait_readable(0.2)
        result = reader.read_nonblock(5)
      end

      Fiber.schedule do
        sleep 0.05
        writer.write("hello")
      end
    end

    reader.close
    writer.close

    assert_equal "hello", result
  end

  # Test that writing to an IO (here a pipe) succeeds immediately
  # when the pipe is ready for writing.
  def test_io_write_immediate
    reader, writer = IO.pipe
    bytes_written = nil

    with_scheduler do |_scheduler|
      Fiber.schedule do
        # If the pipe is empty, write should not block.
        bytes_written = writer.write("test")
      end
    end

    reader.close
    writer.close

    assert_equal 4, bytes_written
  end

  # Test what happens when two fibers are waiting on the same IO.
  # With the current scheduler design, only one fiber will be resumed
  # when data becomes available, and the other will eventually time out.
  def test_multiple_fibers_waiting_on_same_fd
    reader, writer = IO.pipe
    results = []

    with_scheduler do |_scheduler|
      Fiber.schedule do
        res = reader.wait_readable(0.1)
        results << (res ? "readable" : "timeout")
      end

      Fiber.schedule do
        res = reader.wait_readable(0.2)
        results << (res ? "readable" : "timeout")
      end

      Fiber.schedule do
        sleep 0.005
        writer.write("data")
      end
    end

    reader.close
    writer.close

    # One fiber should be resumed with "readable" while the other times out.
    assert_equal 2, results.size
    assert_includes results, "readable"
    assert_includes results, "timeout"
  end
end
