# frozen_string_literal: true
ENV["ITSI_LOG"] = "off"

require "minitest/reporters"
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
  Itsi::Server.start_in_background_thread do
    # Inline Itsi.rb
    bind bind
    workers 1
    threads 1
    fiber_scheduler scheduler_class
    log_level :warn
    run app
  end

  yield URI(bind)
ensure
  Itsi::Server.stop_background_thread
end
