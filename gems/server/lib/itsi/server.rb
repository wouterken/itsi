# frozen_string_literal: true

require_relative "server/version"
require_relative "server/itsi_server"
require_relative "server/rack_interface"
require_relative "server/signal_trap"
require_relative "server/scheduler_interface"
require_relative "server/rack/handler/itsi"
require_relative "server/config"
require_relative "request"
require_relative "stream_io"

# When you Run Itsi without a Rack app,
# we start a tiny little echo server, just so you can see it in action.
DEFAULT_INDEX = IO.read("#{__dir__}/index.html").freeze
DEFAULT_BINDS = ["http://0.0.0.0:3000"].freeze
DEFAULT_APP = lambda {
  require "json"
  require "itsi/scheduler"
  Itsi.log_warn "No config.ru or Itsi.rb app detected. Running default app."
  lambda do |env|
    headers, body = \
      if env["itsi.response"].json?
        [
          { "Content-Type" => "application/json" },
          [{ "message" => "You're running on Itsi!", "rack_env" => env,
             "version" => Itsi::Server::VERSION }.to_json]
        ]
      else
        [
          { "Content-Type" => "text/html" },
          [
            format(
              DEFAULT_INDEX,
              REQUEST_METHOD: env["REQUEST_METHOD"],
              PATH_INFO: env["PATH_INFO"],
              SERVER_NAME: env["SERVER_NAME"],
              SERVER_PORT: env["SERVER_PORT"],
              REMOTE_ADDR: env["REMOTE_ADDR"],
              HTTP_USER_AGENT: env["HTTP_USER_AGENT"]
            )
          ]
        ]
      end
    [200, headers, body]
  end
}

module Itsi
  class Server
    extend RackInterface
    extend SchedulerInterface

    class << self
      def running?
        !!@running
      end

      def build(
        app: nil,
        loader: nil,
        binds: DEFAULT_BINDS,
        **opts
      )
        new(app: loader || -> { app || DEFAULT_APP[] }, binds: binds, **opts)
      end

      def start_in_background_thread(silence: true, **opts)
        start(background: true, silence: silence, **opts)
      end

      def start(background: false, silence: false, **opts)
        build(**opts).tap do |server|
          previous_handler = Signal.trap("INT", "DEFAULT")
          @running = true
          if background
            Thread.new do
              server.start
              @running = false
              Signal.trap("INT", previous_handler)
            end
          else
            server.start
            @running = false
            Signal.trap("INT", previous_handler)
          end
        end
      end
    end
  end
end
