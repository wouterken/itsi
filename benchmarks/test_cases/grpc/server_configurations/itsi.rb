# First attempt to serve incoming requests as static assets,
# falling through to our rack-mapp on not-found.

require_relative "../../../apps/echo_service/echo_service"

grpc EchoServiceImpl.new
