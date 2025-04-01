# frozen_string_literal: true

require_relative "server/version"
require_relative "server/itsi_server"
require_relative "server/rack_interface"
require_relative "server/grpc/grpc_interface"
require_relative "server/grpc/grpc_call"
require_relative "server/scheduler_interface"
require_relative "server/signal_trap"
require_relative "server/route_tester"
require_relative "server/rack/handler/itsi"
require_relative "server/config"
require_relative "standard_headers"
require_relative "http_request"
require_relative "http_response"

module Itsi
  class Server
    extend RackInterface
    extend SchedulerInterface
    extend RouteTester

    class << self
      def running?
        !!@running
      end

      def start_in_background_thread(cli_params = {},
                                     itsi_file = Itsi::Server::Config.config_file_path(cli_params[:config_file]), &blk)
        @background_thread = start(cli_params, itsi_file, background: true, &blk)
      end

      def start(cli_params, itsi_file = Itsi::Server::Config.config_file_path, background: false, &blk)
        server = new(cli_params, itsi_file, blk)
        previous_handler = Signal.trap(:INT, :DEFAULT)
        run = lambda do
          write_pid
          @running = server
          server.start
          @running = false
          Signal.trap(:INT, previous_handler)
          server
        end
        background ? Thread.new(&run) : run[]
      end

      def stop_background_thread
        @running&.stop
        @background_thread&.join
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

        Process.kill(:HUP, pid)
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

        Process.kill(:USR2, pid)
      end

      def load_route_middleware_stack(cli_params)
        Config.build_config(cli_params, Itsi::Server::Config.config_file_path(cli_params[:config_file_path]))[
          "middleware_loader"
          ][]
      end

      def test_route(route_str, cli_params = {})
        matched_route = load_route_middleware_stack(cli_params).find do |route|
          route["route"] =~ route_str
        end
        if matched_route
          print_route(route_str, matched_route)
        else
          puts "No matching route found"
        end
      end

      def routes(cli_params = {})
        load_route_middleware_stack(cli_params).each do |stack|
          routes = explode_route_pattern(stack["route"].source)
          routes.each do |route|
            print_route(route, stack)
          end
        end
        puts "─" * 76
      end
    end
  end
end
