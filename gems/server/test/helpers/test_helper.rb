# frozen_string_literal: true

require "minitest/reporters"

# ENV["ITSI_LOG"] = "off"

require "itsi/server"
require "itsi/scheduler"

Minitest::Reporters.use! Minitest::Reporters::SpecReporter.new

def free_bind(protocol)
  server = TCPServer.new("0.0.0.0", 0)
  port = server.addr[1]
  server.close
  "#{protocol}://0.0.0.0:#{port}"
end

def run_app(app, protocol: "http", bind: free_bind(protocol), scheduler_class: nil)
  server = Itsi::Server.start_in_background_thread({}) do
    bind bind
    workers 1
    threads 1
    fiber_scheduler scheduler_class if scheduler_class
    log_level :error
    run app
  end

  yield URI(bind), server
ensure
  Itsi::Server.stop_background_thread
end
