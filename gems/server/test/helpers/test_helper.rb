# frozen_string_literal: true

require "minitest/reporters"
require "itsi/server"
require "itsi/scheduler"

Minitest::Reporters.use! Minitest::Reporters::SpecReporter.new

def free_bind
  server = TCPServer.new("0.0.0.0", 0)
  port = server.addr[1]
  server.close
  "http://0.0.0.0:#{port}"
end

def run_app(app, **opts)
  bind = free_bind
  server = Itsi::Server.start_in_background_thread(
    app: app,
    binds: [bind],
    **opts
  )

  sleep 0.1
  yield URI(bind), server
ensure
  server&.stop
end
