module Itsi
  class Server
    class GrpcActiveCall
      attr_reader :stream, :method_rpc, :output_metadata, :method_name

      def initialize(stream, method_rpc, service_class, method_name)
        @stream = stream
        @method_rpc = method_rpc
        @service_class = service_class
        @output_metadata = {}
        @method_name = method_name
      end
    end

    class GrpcInterface
      def self.for(services)
        interface = new(services)
        lambda do |request|
          interface.handle_request(request)
        end
      end

      def initialize(services = [])
        @services = {}
        register_services(services)
      end

      def register_services(services)
        services.each do |service|
          service_class = service.class
          @services[service_class.service_name] = {
            implementation: service,
            class: service_class
          }
        end
      end

      def handle_request(request)
        service_info = @services[request.service_name]
        raise "Unknown service: #{request.service_name}" unless service_info

        service_impl = service_info[:implementation]
        service_class = service_info[:class]

        method_rpc = RpcDescWrapper.new(service_class, request.content_type, request.method_name)

        active_call = GrpcActiveCall.new(request.stream, method_rpc, service_class, request.method_name)

        if method_rpc.bidi_streamer?
          puts "Calling bidirectional streaming handler"
          handle_bidi_streaming(active_call, service_impl)
        elsif method_rpc.client_streamer?
          puts "Calling client streaming handler"
          handle_client_streaming(active_call, service_impl)
        elsif method_rpc.server_streamer?
          puts "Calling server streaming handler"
          handle_server_streaming(active_call, service_impl)
        elsif method_rpc.request_response?
          handle_unary(active_call, service_impl)
        end
        active_call.stream.send_trailers({ "grpc-status" => "0" })
      rescue StandardError => e
        active_call.stream.send_trailers({ "grpc-status" => "13", "grpc-message" => e.message })
      end

      private

      # Read a framed message from the stream
      def read_message(stream)
        # Read the gRPC frame header (5 bytes)
        header = stream.read(5)
        return nil if header.nil? || header.bytesize < 5

        compressed = header.bytes[0] == 1
        length = header[1..4].unpack1("N")

        # Read the message body
        stream.read(length)
      end

      # Send a response
      def send_response(active_call, response)
        response_data = active_call.method_rpc.marshal_response(response)
        send_framed_message(active_call.stream, response_data)
      end

      # Send a framed message
      def send_framed_message(stream, message_data, compressed = false)
        compressed_flag = compressed ? 1 : 0
        header = [compressed_flag, message_data.bytesize].pack("CN")

        stream.write(header)
        stream.write(message_data)
        stream.flush
      end

      # Create an enumerator for client streaming requests
      def create_request_enum(active_call)
        Enumerator.new do |yielder|
          loop do
            message_data = read_message(active_call.stream)
            break if message_data.nil?

            request = active_call.method_rpc.unmarshal_request(message_data)
            yielder << request
          end
        end
      end

      # Handlers for different RPC types

      def handle_unary(active_call, service)
        # Read the request message
        message_data = read_message(active_call.stream)
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
        message_data = read_message(active_call.stream)
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
        @rpc_desc = rpc_descs[method_name.to_sym]
        raise "Method not found: #{method_name}" unless @rpc_desc

        @input_type = @rpc_desc.input
        @content_type = content_type
        @input_is_stream = @input_type.is_a?(GRPC::RpcDesc::Stream)
        @input_type = @input_type.type if @input_is_stream

        @output_type = @rpc_desc.output
        @output_is_stream = @output_type.is_a?(GRPC::RpcDesc::Stream)
        @output_type = @output_type.type if @output_is_stream

        @client_streamer = @input_is_stream
        @server_streamer = @output_is_stream
      end

      def client_streamer?
        @client_streamer
      end

      def server_streamer?
        @server_streamer
      end

      def bidi_streamer?
        @client_streamer && @server_streamer
      end

      def request_response?
        !@client_streamer && !@server_streamer
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
