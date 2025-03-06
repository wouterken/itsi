# frozen_string_literal: true

require_relative "server/version"
require_relative "server/itsi_server"
require_relative "request"
require_relative "stream_io"
require_relative "server/rack/handler/itsi"
require 'debug'


class QueueWithTimeout
  def initialize
    @mutex = Mutex.new
    @queue = []
    @received = ConditionVariable.new
  end

  def <<(x)
    @mutex.synchronize do
      @queue << x
      @received.signal
    end
  end

  def push(x)
    self << x
  end

  def pop(non_block = false)
    pop_with_timeout(non_block ? 0 : nil)
  end

  def pop_with_timeout(timeout = nil)
    @mutex.synchronize do
      if timeout.nil? # wait indefinitely until there is an element in the queue
        while @queue.empty?
          @received.wait(@mutex)
        end
      elsif @queue.empty? && timeout != 0 # wait for element or timeout
        timeout_time = timeout + Time.now.to_f
        while @queue.empty? && (remaining_time = timeout_time - Time.now.to_f) > 0
          @received.wait(@mutex, remaining_time)
        end
      end
      #if we're still empty after the timeout, raise exception
      raise ThreadError, "queue empty" if @queue.empty?
      @queue.shift
    end
  end
end


module Itsi
  class Server

    def self.call(app, request)
      respond request, app.call(request.to_env)
    end

    def self.streaming_body?(body)
      body.respond_to?(:call) && !body.respond_to?(:each)
    end

    def self.respond(request, (status, headers, body))
      response = request.response

      # Don't try and respond if we've been hijacked.
      # The hijacker is now responsible for this.
      return if request.hijacked


      # 1. Set Status
      response.status = status

      # 2. Set Headers
      headers.each do |key, value|
        next response.add_header(key, value) unless value.is_a?(Array)

        value.each do |v|
          response.add_header(key, v)
        end
      end

      # 3. Set Body
      # As soon as we start setting the response
      # the server will begin to stream it to the client.

      # If we're partially hijacked or returned a streaming body,
      # stream this response.

      if (body_streamer = streaming_body?(body) ? body : headers.delete("rack.hijack") )
        body_streamer.call(StreamIO.new(response))

      # If we're enumerable with more than one chunk
      # also stream, otherwise write in a single chunk
      elsif body.respond_to?(:each) || body.respond_to?(:to_ary)
        unless body.respond_to?(:each)
          body = body.to_ary
          raise "Body to_ary didn't return an array" unless body.is_a?(Array)
        end
        # We offset this iteration intentionally,
        # to optimize for the case where there's only one chunk.
        buffer = nil
        body.each do |part|
          response.send_frame(buffer.to_s) if buffer
          buffer = part
        end

        response.send_and_close(buffer.to_s)
      else
        response.send_and_close(body.to_s)
      end
    ensure
      response.close_write
      body.close if body.respond_to?(:close)
    end

    def self.start_scheduler_loop(scheduler_class, fiber_proc)
      Fiber.set_scheduler(Kernel.const_get(scheduler_class).new())
      Fiber.schedule(&fiber_proc)
    end

    # If scheduler is enabled
    # Each request is wrapped in a Fiber.
    def self.schedule(app, request)
      Fiber.schedule do
        call(app, request)
      end
    end
  end
end
