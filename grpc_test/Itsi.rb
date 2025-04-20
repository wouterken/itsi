require "grpc"
require_relative 'echo_service_impl'

bind "http://localhost:3000"

grpc EchoServiceImpl.new,
      nonblocking: false,
      compression: 'gzip' do
  # Nested middleware still works:
  response_headers additions: { 'X-Service' => ['Echo'] }
end
