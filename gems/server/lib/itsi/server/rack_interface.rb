module Itsi
  class Server
    module RackInterface
      # Builds a handler proc that is compatible with Rack applications.
      def self.for(app)
        require "rack"
        if app.is_a?(String)
          dir = File.expand_path(File.dirname(app))
          Dir.chdir(dir) do
            loaded_app = ::Rack::Builder.parse_file(File.basename(app))
            app = loaded_app.is_a?(Array) ? loaded_app.first : loaded_app
          end
        end
        lambda do |request|
          Server.respond(request, app.call(env = request.to_rack_env), env)
        end
      end

      # Interface to Rack applications.
      # Here we build the env, and invoke the Rack app's call method.
      # We then turn the Rack response into something Itsi server understands.
      def call(app, request)
        respond request, app.call(env = request.to_rack_env), env
      end

      # Itsi responses are asynchronous and can be streamed.
      # Response chunks are sent using response.send_frame
      # and the response is finished using response.close_write.
      # If only a single chunk is written, you can use the #send_and_close method.
      def respond(request, (status, headers, body), env)
        response = request.response

        # Don't try and respond if we've been hijacked.
        # The hijacker is now responsible for this.
        return if request.hijacked

        # 1. Set Status
        response.status = status

        # 2. Set Headers
        body_streamer = streaming_body?(body) ? body : headers.delete("rack.hijack")

        response.reserve_headers(headers.size)

        for key, value in headers
          case value
          when String then response[key] = value
          when Array
            value.each do |v|
              response[key] = v
            end
          end
        end

        # 3. Set Body
        # As soon as we start setting the response
        # the server will begin to stream it to the client.


        if body_streamer
          # If we're partially hijacked or returned a streaming body,
          # stream this response.
          body_streamer.call(response)

        elsif body.respond_to?(:each) || body.respond_to?(:to_ary)
          # If we're enumerable with more than one chunk
          # also stream, otherwise write in a single chunk
          unless body.respond_to?(:each)
            body = body.to_ary
            raise "Body #to_ary didn't return an array" unless body.is_a?(Array)
          end
          # We offset this iteration intentionally,
          # to optimize for the case where there's only one chunk.
          buffer = nil
          body.each do |part|
            response << buffer.to_s if buffer
            buffer = part
          end

          response.send_and_close(buffer.to_s)
        else
          response.send_and_close(body.to_s)
        end
      rescue EOFError
        response.close
      ensure
        RackEnvPool.checkin(env)
        body.close if body.respond_to?(:close)
      end

      # A streaming body is one that responds to #call and not #each.
      def streaming_body?(body)
        body.respond_to?(:call) && !body.respond_to?(:each)
      end
    end
  end
end
