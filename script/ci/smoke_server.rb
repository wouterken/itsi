# frozen_string_literal: true

require "itsi/server"
require "net/http"
require "socket"
require "uri"

def free_bind
  server = TCPServer.new("127.0.0.1", 0)
  port = server.addr[1]
  server.close
  "http://127.0.0.1:#{port}"
end

bind = free_bind
uri = URI("#{bind}/health")
sync = Queue.new

Itsi::Server.start_in_background_thread(
  binds: [bind],
  hooks: { "after_start" => -> { sync << true } }
) do
  workers 1
  threads 1
  log_level :warn
  run lambda { |_env| [200, { "content-type" => "text/plain" }, ["ok"]] }
end

begin
  sync.pop
  response = Net::HTTP.get_response(uri)
  raise "Unexpected response code #{response.code}" unless response.code == "200"
  raise "Unexpected response body #{response.body.inspect}" unless response.body == "ok"
ensure
  Itsi::Server.stop_background_threads
end

puts "server smoke ok"
