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
require_relative "server/typed_handlers"
require_relative "standard_headers"
require_relative "http_request"
require_relative "http_response"
require_relative "passfile"
require_relative "../shell_completions/completions"

module Itsi
  class Server
    extend RackInterface
    extend SchedulerInterface
    extend RouteTester

    class << self

      def running?
        !@running || @running.empty?
      end

      def start_in_background_thread(cli_params = {}, &blk)
        @background_thread ||= []
        server, background_thread = start(cli_params, background: true, &blk)
        @background_thread << background_thread
        server
      end

      def start(cli_params, background: false, &blk)
        itsi_file = Itsi::Server::Config.config_file_path(cli_params[:config_file])
        server = new(cli_params, itsi_file, blk)
        previous_handler = Signal.trap(:INT, :DEFAULT)
        run = lambda do
          @running ||= []
          @running << server

          if cli_params[:daemonize]
            Itsi.log_info("Itsi is running in the background. Writing pid to #{Itsi::Server::Config.pid_file_path}")
            Itsi.log_info("To stop Itsi, run 'itsi stop' from this directory.")
            Process.daemon(true, false)
          end
          write_pid

          server.start
          @running.delete(server)
          Signal.trap(:INT, previous_handler)
          server
        end
        background ? [server, Thread.new(&run)] : run[]
      rescue
      end

      def static(cli_params)
        start(cli_params.merge(static: true))
      end

      def stop
        return unless pid = get_pid
        Process.kill(:INT, pid)
        i = 0
        while i < 10
          sleep 0.25
          unless get_pid(false)
            puts "Itsi stopped"
            break
          end
          i += 1
        end
      end

      def stop_background_threads
        @running && @running.each(&:stop)
        @background_threads&.each(&:join)
        @background_threads = []
        @running = []
      end

      def write_pid
        File.write(Itsi::Server::Config.pid_file_path, Process.pid)
      end

      def get_pid(warn=true)
        pid = File.read(Itsi::Server::Config.pid_file_path).to_i
        if Process.kill(0, pid)
          pid
        else
          warn ? puts("No server running") : nil
          nil
        end
      rescue StandardError
        warn ? puts("No server running") : nil
        nil
      end

      def test
        Itsi::Server::Config.test!(cli_params = {})
      end

      def init
        Config.write_default
      end

      def reload
        return unless pid = get_pid

        Process.kill(:HUP, pid)
      end

      def restart
        return unless pid = get_pid

        Process.kill(:USR1, pid)
      end

      def passfile(options, subcmd)
        filename = options[:passfile]
        unless filename
          puts "Error: passfile not set. Use --passfile option to provide a path to a file containing hashed credentials."
          puts "This file contains hashed credentials and should not be included in source control without additional protection."
          exit(1)
        end
        algorithm = options.fetch(:algorithm, 'sha256')

        unless %w[sha256 sha512 bcrypt argon2 none].include?(algorithm)
          puts "Invalid algorithm"
          exit(1)
        end

        case subcmd
        when 'add', 'echo'
          Passfile.send(subcmd, filename, algorithm)
        when 'remove', 'list'
          Passfile.send(subcmd, filename)
        else
          puts "Valid subcommands are: add | remove | list"
          exit(0)
        end
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
        Itsi.log_info("Itsi running on #{pid}")
        Process.kill(:USR2, pid)
      end

      def load_route_middleware_stack(cli_params)
        Config.build_config(cli_params, Itsi::Server::Config.config_file_path(cli_params[:config_file_path])).first[
          "middleware_loader"
          ][]
      end

      def test_route(cli_params, route_str)
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

      alias serve start

    end
  end
end
