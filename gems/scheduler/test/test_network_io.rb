# frozen_string_literal: true

require_relative "test_helper"
require "socket"
require "timeout"
require "debug"

class TestNetworkIO < Minitest::Test
  include Itsi::Scheduler::TestHelper

  def test_tcp_echo
    message = "Hello, Itsi!"
    response = nil

    with_scheduler do |_scheduler|
      server = TCPServer.new("127.0.0.1", 0)
      port = server.addr[1]

      # Server fiber: accept one connection and echo data.
      Fiber.schedule do
        client = server.accept
        data = client.read(message.size)
        client.write(data)
        client.close
        server.close
      end

      # Client fiber: connect, send message, and read echo.
      Fiber.schedule do
        client = TCPSocket.new("127.0.0.1", port)
        client.write(message)
        response = client.read(message.size)
        client.close
      end
    end

    assert_equal message, response
  end

  def test_concurrent_tcp_clients
    server = TCPServer.new("127.0.0.1", 0)
    port = server.addr[1]
    messages = %w[first second third fourth]
    responses = {}

    with_scheduler do |_scheduler|
      # Server fiber: accept several connections.
      Fiber.schedule do
        messages.size.times do
          client = server.accept
          # Spawn a fiber for each connection to echo the data.
          Fiber.schedule do
            data = client.readpartial(1024)
            client.write(data)
            client.close
          end
        end
        server.close
      end

      # Client fibers: connect concurrently and send messages.
      messages.each do |msg|
        Fiber.schedule do
          sleep rand(0.01..0.05) # random delay for interleaving
          client = TCPSocket.new("127.0.0.1", port)
          client.write(msg)
          responses[msg] = client.read(msg.size)
          client.close
        end
      end
    end

    messages.each do |msg|
      assert_equal msg, responses[msg]
    end
  end

  def test_interleaved_network_and_sleep
    server = TCPServer.new("127.0.0.1", 0)
    port = server.addr[1]
    responses = {}

    with_scheduler do |_scheduler|
      # Echo server that delays between chunks.
      Fiber.schedule do
        client = server.accept
        data = "".dup
        # Read in chunks until we have received 12 bytes.
        while data.size < 12
          begin
            chunk = client.readpartial(3)
          rescue EOFError
            break
          end
          data << chunk
          sleep 0.02 # delay between chunks
        end
        # Echo back the reversed data.
        client.write(data.reverse)
        client.close
        server.close
      end

      # Client fiber: send data in small chunks.
      Fiber.schedule do
        sleep 0.05 # allow server to start
        client = TCPSocket.new("127.0.0.1", port)
        "HelloWorld!!".chars.each_slice(3) do |slice|
          client.write(slice.join)
          sleep 0.01
        end
        responses[:result] = client.read(12)
        client.close
      end
    end

    expected = "HelloWorld!!".reverse[0, 12]
    assert_equal expected, responses[:result]
  end

  def test_tcp_timeout
    server = TCPServer.new("127.0.0.1", 0)
    port = server.addr[1]
    result = nil

    with_scheduler do |_scheduler|
      # Server fiber: accept the connection but delay sending.
      Fiber.schedule do
        client = server.accept
        sleep 0.2 # delay long enough to force a timeout on the client
        client.write("late data")
        client.close
        server.close
      end

      # Client fiber: connect and wait for data with a short timeout.
      Fiber.schedule do
        sleep 0.05
        client = TCPSocket.new("127.0.0.1", port)
        result = if client.wait_readable(0.1)
                   client.readpartial(1024)
                 else
                   nil
                 end
        client.close
      end
    end

    # The client should time out (i.e. result remains nil) because the server waits too long.
    assert_nil result
  end

  def test_multiple_fibers_on_same_socket
    server = TCPServer.new("127.0.0.1", 0)
    port = server.addr[1]
    results = []

    with_scheduler do |_scheduler|
      # Server fiber: accept a connection and send a short message.
      Fiber.schedule do
        client1 = server.accept
        client2 = server.accept
        client1.write("network")
        client2.write("network")
        client1.close
        client2.close
        server.close
      end

      # Two separate client fibers using separate connections.
      # (In many schedulers, if two fibers wait on the same IO object,
      # only one may be resumed when data becomes available.)
      2.times do |i|
        Fiber.schedule do
          sleep 0.001
          client = TCPSocket.new("127.0.0.1", port)
          res = client.wait_readable(0.05) ? "readable" : "timeout"
          results << res
          client.close
        end
      end
    end

    assert_equal 2, results.size
    assert_includes results, "readable"
  end

  def test_two_fibers_only_one_connects
    server = TCPServer.new("127.0.0.1", 0)
    port = server.addr[1]
    results = []

    with_scheduler do |_scheduler|
      # Server fiber: accept a connection and send a short message.
      Fiber.schedule do
        client1 = server.accept
        client1.write("network")
        client1.close
        sleep 0.1
        server.close
      end

      # Two separate client fibers using separate connections.
      # (In many schedulers, if two fibers wait on the same IO object,
      # only one may be resumed when data becomes available.)
      2.times do |i|
        Fiber.schedule do
          sleep 0.001
          client = TCPSocket.new("127.0.0.1", port)
          res = client.wait_readable(0.05) ? "readable" : "timeout"
          results << res
          client.close
        end
      end
    end

    assert_equal 2, results.size
    assert_includes results, "readable"
    assert_includes results, "timeout"
  end

  def test_udp_and_tcp_interleaving
    # Set up a UDP server.
    udp_server = UDPSocket.new
    udp_server.bind("127.0.0.1", 0)
    udp_port = udp_server.addr[1]

    # Set up a TCP server.
    tcp_server = TCPServer.new("127.0.0.1", 0)
    tcp_port = tcp_server.addr[1]

    udp_result = nil
    tcp_result = nil

    with_scheduler do |_scheduler|
      # UDP server fiber: wait for a datagram.
      Fiber.schedule do
        udp_result = udp_server.recvfrom(1024)[0]
        udp_server.close
      end

      # TCP server fiber: accept a connection and read data.
      Fiber.schedule do
        client = tcp_server.accept
        tcp_result = client.readpartial(1024)
        client.close
        tcp_server.close
      end

      # UDP client fiber: send a datagram.
      Fiber.schedule do
        sleep 0.02
        udp_client = UDPSocket.new
        udp_client.send("udp data", 0, "127.0.0.1", udp_port)
        udp_client.close
      end

      # TCP client fiber: connect and send data.
      Fiber.schedule do
        sleep 0.02
        client = TCPSocket.new("127.0.0.1", tcp_port)
        client.write("tcp data")
        client.close
      end

      # Additional fiber to interleave a sleep.
      Fiber.schedule do
        sleep 0.05
      end
    end

    assert_equal "udp data", udp_result
    assert_equal "tcp data", tcp_result
  end
end
