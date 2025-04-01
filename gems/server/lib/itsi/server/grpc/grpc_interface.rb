module Itsi
  class Server
    class GrpcInterface
      DeadlineExceeded = Class.new(StandardError)

      attr_accessor :service, :service_class

      def self.for(service)
        interface = new(service)
        lambda do |request|
          interface.handle_request(request)
        end
      end

      def self.reflection_for(handlers)
        require_relative "reflection/v1/reflection_services_pb"
        interface = new(Grpc::Reflection::V1::ServerReflection::Service.new(handlers))
        lambda do |request|
          interface.handle_request(request)
        end
      end

      def initialize(service)
        @service = service
        @service_class = service.class
        @service_class.rpc_descs.transform_keys! do |k|
          k.to_s.gsub(/([a-z])([A-Z])/, "\\1_\\2").downcase.to_sym
        end
      end

      def handle_request(active_call)
        unless (active_call.rpc_desc = service_class.rpc_descs[active_call.method_name])
          active_call.stream.write("\n")
          active_call.send_status(13, "Method not found")
          active_call.close
          return
        end

        active_call.send_initial_metadata(
          {
            "grpc-accept-encoding" => "gzip, deflate, identity",
            "content-type" => active_call.json? ? "application/json" : "application/grpc"
          }
        )

        begin
          if active_call.bidi_streamer?
            handle_bidi_streaming(active_call)
          elsif active_call.client_streamer?
            handle_client_streaming(active_call)
          elsif active_call.server_streamer?
            handle_server_streaming(active_call)
          elsif active_call.request_response?
            handle_unary(active_call)
          end
          active_call.send_status(0, "Success")
        rescue Google::Protobuf::ParseError => e
          active_call.send_empty
          active_call.send_status(3, e.message)
        rescue DeadlineExceeded => e
          active_call.send_empty
          active_call.send_status(4, e.message)
        rescue StandardError => e
          active_call.send_empty
          active_call.send_status(13, e.message)
        end
      rescue StandardError => e
        Itsi.log_warn("Unhandled error in grpc_interface: #{e.message}")
      ensure
        active_call.close
      end

      private

      def handle_unary(active_call)
        message = active_call.remote_read
        response = service.send(active_call.method_name, message, active_call)
        active_call.remote_send(response)
      end

      def handle_client_streaming(active_call)
        response = service.send(active_call.method_name, active_call.each_remote_read, active_call)
        active_call.remote_send(response)
      end

      def handle_server_streaming(active_call)
        message = active_call.remote_read
        service.send(active_call.method_name, message, active_call) do |response|
          active_call.remote_send(response)
        end
      end

      def handle_bidi_streaming(active_call)
        service.send(active_call.method_name, active_call.each_remote_read, active_call) do |response|
          active_call.remote_send(response)
        end
      end
    end
  end
end
