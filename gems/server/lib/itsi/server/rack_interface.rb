module Itsi
  class Server
    module RackInterface
      # Interface to Rack applications.
      # Here we build the env, and invoke the Rack app's call method.
      # We then turn the Rack response into something Itsi server understands.
      def call(app, request)
        respond request, app.call(request.to_rack_env)
      end

      # Itsi responses are asynchronous and can be streamed.
      # Response chunks are sent using response.send_frame
      # and the response is finished using response.close_write.
      # If only a single chunk is written, you can use the #send_and_close method.
      def respond(request, (status, headers, body))
        response = request.response

        # Don't try and respond if we've been hijacked.
        # The hijacker is now responsible for this.
        return if request.hijacked

        # 1. Set Status
        response.status = status

        # 2. Set Headers
        body_streamer = streaming_body?(body) ? body : headers.delete("rack.hijack")
        headers.each do |key, value|
          unless value.is_a?(Array)
            response[key] = value
            next
          end

          value.each do |v|
            response[key] = v
          end
        end

        # 3. Set Body
        # As soon as we start setting the response
        # the server will begin to stream it to the client.

        # If we're partially hijacked or returned a streaming body,
        # stream this response.

        if body_streamer
          body_streamer.call(response)

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
            response << buffer.to_s if buffer
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

      # A streaming body is one that responds to #call and not #each.
      def streaming_body?(body)
        body.respond_to?(:call) && !body.respond_to?(:each)
      end
    end
  end
end
