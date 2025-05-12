require_relative "./apps/echo_service/echo_service"

def main
  thread_count = (ENV['THREADS'] || 30).to_i

  server = GRPC::RpcServer.new(pool_size: thread_count)
  server.add_http2_port("0.0.0.0:#{ENV['PORT'] || 50051}", :this_port_is_insecure)
  server.handle(EchoServiceImpl)
  server.run_till_terminated
end

main
