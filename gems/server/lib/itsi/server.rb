# frozen_string_literal: true

require_relative "server/version"
require_relative "server/itsi_server"
require_relative "signals"
require_relative "request"
require_relative "stream_io"
require_relative "server/rack/handler/itsi"
require 'erb'

DEFAULT_INDEX = IO.read(__dir__ + '/index.html.erb')

module Itsi
  class Server

    def self.running?
      @running ||= false
    end

    def self.start(
      app: ->(env){
        [env['CONTENT_TYPE'], env['HTTP_ACCEPT']].include?('application/json') ?
          [200, {"Content-Type" => "application/json"}, ["{\"message\": \"You're running on Itsi!\"}"]] :
          [200, {"Content-Type" => "text/html"}, [
            DEFAULT_INDEX % {
              REQUEST_METHOD: env['REQUEST_METHOD'],
              PATH_INFO: env['PATH_INFO'],
              SERVER_NAME: env['SERVER_NAME'],
              SERVER_PORT: env['SERVER_PORT'],
              REMOTE_ADDR: env['REMOTE_ADDR'],
              HTTP_USER_AGENT: env['HTTP_USER_AGENT']
            }
          ]]
      },
      binds: ['http://0.0.0.0:3000'],
      **opts
    )
      server = new(app: ->{app}, binds: binds, **opts)
      @running = true
      Signal.trap('INT', 'DEFAULT')
      server.start
    ensure
      @running = false
    end

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

      if (body_streamer = streaming_body?(body) ? body : headers.delete("rack.hijack"))
        body_streamer.call(StreamIO.new(response))

      # If we're enumerable with more than one chunk
      # also stream, otherwise write in a single chunk
      elsif body.respond_to?(:each) || body.respond_to?(:to_ary)
        unless body.respond_to?(:each)
          body = body.to_ary
          raise "Body #to_ary didn't return an array" unless body.is_a?(Array)
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

    def self.start_scheduler_loop(scheduler_class, scheduler_task)
      scheduler = scheduler_class.new
      Fiber.set_scheduler(scheduler)
      [scheduler, Fiber.schedule(&scheduler_task)]
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
