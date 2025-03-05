# frozen_string_literal: true

require_relative "server/version"
require_relative "server/itsi_server"
require_relative "request"
require_relative "stream_io"

module Itsi
  class Server
    def self.call(app, request)
      respond(request, app.call(request.to_env))
    end

    def self.streaming_body?(body)
      body.respond_to?(:call) && !body.respond_to?(:each)
    end

    def self.respond(request, (status, headers, body))
      response = request.response

      return if request.hijacked

      # Set HTTP Status
      response.status = status

      # Set Headers
      headers.each do |key, value|
        next response.add_header(key, value) unless value.is_a?(Array)

        value.each do |v|
          response.add_header(key, v)
        end
      end

      # If we're partially hijacked or returned a streaming body,
      # stream this response
      if (body_streamer = streaming_body?(body) ? body : headers.delete("rack.hijack") )
        body_streamer.call(StreamIO.new(response))

      # If we're enumerable with more than one chunk
      # also stream, otherwise write in a single chunk
      elsif body.respond_to?(:each) || body.respond_to?(:to_ary)
        unless body.respond_to?(:each)
          body = body.to_ary
          raise "Body to_ary didn't return an array" unless body.is_a?(Array)
        end
        buffer = nil
        body.each do |part|
          response.send_frame(buffer) if buffer
          buffer = part.to_s
        end

        response.send_and_close(buffer)
      else
        response.send_and_close(body.to_s)
      end
    ensure
      response.close_write
      body.close if body.respond_to?(:close)
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
