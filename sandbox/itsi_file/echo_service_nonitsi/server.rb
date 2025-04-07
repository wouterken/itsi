require 'grpc'
require_relative 'echo_services_pb'
require_relative 'echo_pb' # Your generated file

# Define the EchoService implementation
class EchoService < Echo::EchoService::Service
  # Implement the Echo method
  def echo(request, _unused_call)
    Echo::EchoResponse.new(message: "Echo: #{request.message}", count: 1)
  end

  # Implement the EchoStream method (streaming response)
  def echo_stream(request, _unused_call)
    Enumerator.new do |yielder|
      3.times do |i|
        yielder << Echo::EchoResponse.new(message: "#{request.message} - part #{i + 1}", count: i + 1)
        sleep 0.5 # Simulate processing time
      end
    end
  end

  # Implement the EchoCollect method (streaming request)
  def echo_collect(call)
    count = 0
    message = ''
    call.each_remote_read do |req|
      count += 1
      message += req.message + ' '
    end
    Echo::EchoResponse.new(message: message.strip, count: count)
  end

  # Implement the EchoBidirectional method (bidirectional streaming)
  def echo_bidirectional(requests, _unused_call)
    Enumerator.new do |yielder|
      requests.each do |req|
        yielder << Echo::EchoResponse.new(message: "Echoing back: #{req.message}", count: req.message.length)
      end
    end
  end
end

# Create the gRPC server
def main
  server = GRPC::RpcServer.new
  server.add_http2_port('0.0.0.0:50051', :this_port_is_insecure)
  server.handle(EchoService)
  puts 'Server running on 0.0.0.0:50051'
  server.run_till_terminated
end

main if __FILE__ == $PROGRAM_NAME
