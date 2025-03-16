# frozen_string_literal: true

require "minitest/reporters"

ENV['ITSI_LOG'] = 'off'

require "itsi/server"
require "itsi/scheduler"

Minitest::Reporters.use! Minitest::Reporters::SpecReporter.new

def free_bind(protocol)
  server = TCPServer.new("0.0.0.0", 0)
  port = server.addr[1]
  server.close
  "#{protocol}://0.0.0.0:#{port}"
end

def run_app(app, protocol: "http", bind: free_bind(protocol), **opts)
  server = Itsi::Server.start_in_background_thread(
    app: app,
    binds: [bind],
    **opts
  )

  sleep 0.005
  yield URI(bind), server
ensure
  server&.stop
end
