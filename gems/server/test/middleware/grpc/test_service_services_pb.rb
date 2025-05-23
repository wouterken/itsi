# Generated by the protocol buffer compiler.  DO NOT EDIT!
# Source: test_service.proto for package 'test'

require 'grpc'
require_relative 'test_service_pb'

module Test
  module TestService

    class Service

      include ::GRPC::GenericService

      self.marshal_class_method = :encode
      self.unmarshal_class_method = :decode
      self.service_name = 'test.TestService'

      # Unary RPC
      rpc :UnaryEcho, ::Test::EchoRequest, ::Test::EchoResponse
      # Client‑streaming RPC
      rpc :ClientStream, stream(::Test::StreamRequest), ::Test::StreamResponse
      # Server‑streaming RPC
      rpc :ServerStream, ::Test::EchoRequest, stream(::Test::StreamResponse)
      # Bidirectional streaming RPC
      rpc :BidiStream, stream(::Test::EchoRequest), stream(::Test::EchoResponse)
    end

    Stub = Service.rpc_stub_class
  end
end
