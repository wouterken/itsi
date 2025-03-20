# frozen_string_literal: true

require_relative "server/version"
require_relative "server/itsi_server"
require_relative "server/rack_interface"
require_relative "server/signal_trap"
require_relative "server/scheduler_interface"
require_relative "server/rack/handler/itsi"
require_relative "server/config"
require_relative "http_request"

module Itsi
  class Server
    extend RackInterface
    extend SchedulerInterface

    class << self
      def running?
        !!@running
      end

      def start_in_background_thread(*args)
        start(*args, background: true)
      end

      def start(*args, background: false)
        new(*args).tap do |server|
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
