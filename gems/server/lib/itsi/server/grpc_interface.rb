module Itsi
  class Server
    class GrpcActiveCall
      attr_reader :stream, :reader, :method_rpc, :output_metadata, :method_name, :content_type

      def initialize(stream, content_type, method_rpc, service_class, method_name)
        @stream = stream
        @content_type = content_type
        reader_fileno = stream.reader
        @reader = IO.open(reader_fileno, 'rb')
        @method_rpc = method_rpc
        @service_class = service_class
        @output_metadata = {}
        @method_name = method_name
      end

      def close
        @reader&.close
      end
    end

    class GrpcInterface
      def self.for(service)
        interface = new(service)
        lambda do |request|
          interface.handle_request(request)
        end
      end

      def initialize(service)
        @service = service
        @service_class = service.class
      end

      def handle_request(request)
        method_rpc = RpcDescWrapper.new(@service_class, request.content_type, request.method_name)
        active_call = GrpcActiveCall.new(request.stream, request.content_type, method_rpc, @service_class, request.method_name)

        if method_rpc.bidi_streamer?
          handle_bidi_streaming(active_call, @service)
        elsif method_rpc.client_streamer?
          handle_client_streaming(active_call, @service)
        elsif method_rpc.server_streamer?
          handle_server_streaming(active_call, @service)
        elsif method_rpc.request_response?
          handle_unary(active_call, @service)
        end
        request.stream.send_trailers({ "grpc-status" => "0" })
      rescue Google::Protobuf::ParseError => e
        puts e
        request.stream&.send_trailers({ "grpc-status" => "3", "grpc-message" => e.message })
      rescue StandardError => e
        puts e
        request.stream&.send_trailers({ "grpc-status" => "13", "grpc-message" => e.message })
      ensure
        active_call&.close
      end

      private

      # Read a framed message from the stream
      def read_message(active_call)
        if active_call.content_type == "application/json"
          active_call.reader.read
        else
          # Read the gRPC frame header (5 bytes)
          header = active_call.reader.read(5)
          return nil if header.nil? || header.bytesize < 5

          compressed = header.bytes[0] == 1
          length = header[1..4].unpack1("N")

          # Read the message body
          active_call.reader.read(length)
        end
      end

      # Send a response
      def send_response(active_call, response)
        response_data = active_call.method_rpc.marshal_response(response)
        send_framed_message(active_call, response_data)
      end

      # Send a framed message
      def send_framed_message(active_call, message_data, compressed = false)
        if active_call.content_type == "application/json"
          active_call.stream.write(message_data)
        else
          compressed_flag = compressed ? 1 : 0
          header = [compressed_flag, message_data.bytesize].pack("CN")

          active_call.stream.write(header)
          active_call.stream.write(message_data)
          active_call.stream.flush
        end
      end

      # Create an enumerator for client streaming requests
      def create_request_enum(active_call)
        Enumerator.new do |yielder|
          loop do
            message_data = read_message(active_call)
            break if message_data.nil?

            request = active_call.method_rpc.unmarshal_request(message_data)
            yielder << request
          end
        end
      end

      # Handlers for different RPC types

      def handle_unary(active_call, service)
        # Read the request message
        message_data = read_message(active_call)
        request = active_call.method_rpc.unmarshal_request(message_data)

        # Call the service implementation
        underscore_method = GRPC::GenericService.underscore(active_call.method_name)
        response = service.send(underscore_method, request, active_call)

        # Send response
        send_response(active_call, response)
      end

      def handle_client_streaming(active_call, service)
        # Create an enumerable to read the incoming stream
        request_enum = create_request_enum(active_call)

        # Call the service implementation
        underscore_method = GRPC::GenericService.underscore(active_call.method_name)
        response = service.send(underscore_method, request_enum, active_call)

        # Send response
        send_response(active_call, response)
      end

      def handle_server_streaming(active_call, service)
        # Read the request message
        message_data = read_message(active_call)
        request = active_call.method_rpc.unmarshal_request(message_data)

        # Call the service implementation with a block to handle streaming
        underscore_method = GRPC::GenericService.underscore(active_call.method_name)
        service.send(underscore_method, request, active_call) do |response|
          send_response(active_call, response)
        end
      end

      def handle_bidi_streaming(active_call, service)
        # Create an enumerable to read the incoming stream
        request_enum = create_request_enum(active_call)

        # Call the service implementation with a block
        underscore_method = GRPC::GenericService.underscore(active_call.method_name)
        service.send(underscore_method, request_enum, active_call) do |response|
          send_response(active_call, response)
        end
      end
    end

    class RpcDescWrapper
      def initialize(service_class, content_type, method_name)
        rpc_descs = service_class.rpc_descs
        @content_type = content_type
        @rpc_desc = rpc_descs[method_name.to_sym]
        raise "Method not found: #{method_name}" unless @rpc_desc

        @input_type = @rpc_desc.input
        @input_is_stream = @input_type.is_a?(GRPC::RpcDesc::Stream)
        @input_type = @input_type.type if @input_is_stream

        @output_type = @rpc_desc.output
        @output_is_stream = @output_type.is_a?(GRPC::RpcDesc::Stream)
        @output_type = @output_type.type if @output_is_stream

      end

      def client_streamer?
        @input_is_stream
      end

      def server_streamer?
        @output_is_stream
      end

      def bidi_streamer?
        @input_is_stream && @output_is_stream
      end

      def request_response?
        !@input_is_stream && !@output_is_stream
      end

      def unmarshal_request(data)
        if @content_type == "application/json"
          @input_type.decode_json(data)
        else
          @input_type.decode(data)
        end
      end

      def marshal_response(response)
        if @content_type == "application/json"
          response.to_json
        else
          response.to_proto
        end
      end
    end
  end
end
