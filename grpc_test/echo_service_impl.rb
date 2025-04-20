require_relative 'echo_services_pb'

class EchoServiceImpl < Echo::EchoService::Service
  # Unary RPC implementation
  def echo(req, _unused_call)
    Echo::EchoResponse.new(message: req.message)
  end
end
