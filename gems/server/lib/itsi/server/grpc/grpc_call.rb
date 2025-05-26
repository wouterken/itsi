# frozen_string_literal: true

require "stringio"

module Itsi
  class GrpcCall
    attr_accessor :rpc_desc

    def input_stream?
      @input_stream ||= @rpc_desc&.input.is_a?(GRPC::RpcDesc::Stream) || false
    end

    def output_stream?
      @output_stream ||= @rpc_desc&.output.is_a?(GRPC::RpcDesc::Stream) || false
    end

    def input_type
      @input_type ||= input_stream? ? rpc_desc.input.type : rpc_desc.input
    end

    def output_type
      @output_type ||= output_stream? ? rpc_desc.output.type : rpc_desc.output
    end

    def reader
      @reader ||= IO.open(stream.reader_fileno, "rb")
    end

    def close
      if output_stream? && content_type == "application/json"
        stream.write("[") unless @opened
        stream.write("]")
      end

      @reader&.close
      stream.close
    end

    def deadline
      return @deadline if defined?(@deadline)
      return @deadline = nil unless timeout

      @deadline = Time.now + timeout
    end

    def parse_from_json_stream(json_stream)
      first_char = nil
      loop do
        char = json_stream.read(1)
        break if char.nil?
        if char =~ /\s/
          next
        elsif ["[", ","].include?(char)
          first_char = char
          break
        elsif char == "]"
          return nil
        else
          # If the first non-whitespace character is not '[' or comma, return nil.
          return nil
        end
      end

      return nil if first_char.nil?

      # Step 2: Process objects until we hit the end of the JSON stream or array.
      loop do # rubocop:disable Lint/UnreachableLoop,Metrics/BlockLength
        # Skip any whitespace or commas preceding an object.
        char = nil
        loop do
          char = json_stream.read(1)
          break if char.nil?
          next if char =~ /\s/

          break
        end
        # The next non-whitespace, non-comma character should be the start of an object.
        return nil unless char == "{"

        # Step 3: Start buffering the JSON object.
        buffer = "{".dup
        stack = ["{"]
        in_string = false
        escape = false

        while stack.any?
          ch = json_stream.read(1)
          return nil if ch.nil? # premature end of stream

          buffer << ch

          if in_string
            if escape
              escape = false
              next
            end
            if ch == "\\"
              escape = true
            elsif ch == '"'
              in_string = false
            end
          elsif ch == '"'
            in_string = true
          elsif ["{", "["].include?(ch)
            stack.push(ch)
          elsif ["}", "]"].include?(ch)
            expected = (ch == "}" ? "{" : "[")
            # Check for matching bracket.
            return nil unless stack.last == expected

            stack.pop
          end
        end
        # Yield the complete JSON object (as a string).
        return buffer
      end
    end

    def remote_read
      if content_type == "application/json"
        if input_stream?
          if (next_item = parse_from_json_stream(reader))
            input_type.decode_json(next_item)
          end
        else
          input_type.decode_json(reader.read)
        end
      else
        header = reader.read(5)
        return nil if header.nil? || header.bytesize < 5

        compressed = header.bytes[0] == 1
        length = header[1..4].unpack1("N")

        data = reader.read(length)
        return nil if data.nil?

        data = decompress_input(data) if compressed

        input_type.decode(data)
      end
    end

    def send_framed_message(message_data, compressed = nil)
      if content_type == "application/json"
        if output_stream?
          if @opened
            stream.write(",\n")
          else
            stream.write("[")
            @opened = true
          end
        end
        message_data = output_type.encode_json(message_data)
        stream.write(message_data)
      else
        message_data = output_type.encode(message_data)
        should_compress = compressed.nil? ? should_compress_output?(message_data.bytesize) : compressed

        if should_compress
          message_data = compress_output(message_data)
          compressed_flag = 1
        else
          compressed_flag = 0
        end

        message = [compressed_flag, message_data.bytesize].pack("CN") << message_data
        stream.write(message)

        @body_written = true
      end
    rescue IOError
      close
    end

    def remote_send(response)
      send_framed_message(response)
    end

    def deadline_exceeded?
      @deadline && @deadline <= Time.now
    end

    def each_remote_read
      return enum_for(:each_remote_read) unless block_given?

      while (resp = remote_read) || !cancelled?

        yield resp
      end
    end

    alias_method :each, :each_remote_read

    def bidi_streamer?
      input_stream? && output_stream?
    end

    def client_streamer?
      input_stream? && !output_stream?
    end

    def server_streamer?
      !input_stream? && output_stream?
    end

    def request_response?
      !input_stream? && !output_stream?
    end

    def send_initial_metadata(kv_pairs)
      add_headers(kv_pairs.transform_values { |v| Array(v) })
    end

    def send_empty
      remote_send(output_type.new) unless @body_written
    end

    def send_status(status_code, status_details, trailing_metadata = {})
      trailers = {
        "grpc-status" => status_code.to_s
      }

      unless status_details.nil? || status_details.empty?
        encoded_message = status_details.gsub(/[ #%&+,:;<=>?@\[\]^`{|}~\\"]/) do |c|
          "%" + c.ord.to_s(16).upcase
        end
        trailers["grpc-message"] = encoded_message
      end

      trailing_metadata.each do |key, value|
        trailers[key] = \
          if key.to_s.end_with?("-bin")
            Base64.strict_encode64(value.to_s)
          else
            value.to_s
          end
      end

      send_trailers(trailers)
    end

    def send_trailers(trailers)
      stream.send_trailers(trailers)
    end
  end
end
