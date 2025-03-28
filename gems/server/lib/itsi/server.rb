# frozen_string_literal: true

require_relative "server/version"
require_relative "server/itsi_server"
require_relative "server/rack_interface"
require_relative "server/grpc_interface"
require_relative "server/scheduler_interface"
require_relative "server/signal_trap"
require_relative "server/rack/handler/itsi"
require_relative "server/config"
require_relative "standard_headers"
require_relative "http_request"
require_relative "http_response"

module Itsi
  class Server
    extend RackInterface
    extend SchedulerInterface

    class << self
      def running?
        !!@running
      end

      def start_in_background_thread(cli_params, itsi_file = Itsi::Server::Config.config_file_path)
        start(cli_params, itsi_file, background: true)
      end

      def start(cli_params, itsi_file = Itsi::Server::Config.config_file_path, background: false)
        new(cli_params, itsi_file).tap do |server|
          previous_handler = Signal.trap(:INT, :DEFAULT)
          @running = true
          run = lambda do
            write_pid
            server.start
            @running = false
            Signal.trap(:INT, previous_handler)
          end
          background ? Thread.new(&run) : run[]
        end
      end

      def write_pid
        File.write(Itsi::Server::Config.pid_file_path, Process.pid)
      end

      def get_pid
        pid = File.read(Itsi::Server::Config.pid_file_path).to_i
        if Process.kill(0, pid)
          pid
        else
          nil
        end
      rescue StandardError
        nil
      end

      def reload
        return unless pid = get_pid

        Process.kill(:USR2, pid)
      end

      def restart
        return unless pid = get_pid

        Process.kill(:USR1, pid)
      end

      def down
        puts :down
      end

      def up
        puts :up
      end

      def add_worker
        return unless pid = get_pid

        Process.kill(:TTIN, pid)
      end

      def remove_worker
        return unless pid = get_pid

        Process.kill(:TTOU, pid)
      end

      def status
        return unless pid = get_pid

        Process.kill(:INFO, pid)
      end
    end
  end
end
