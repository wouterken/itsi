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

require "securerandom"
require "fileutils"

module Itsi
  class Server
    extend RackInterface
    extend SchedulerInterface
    extend RouteTester

    class << self
      def running?
        @running && !@running.empty?
      end

      def start_in_background_thread(cli_params = {}, &blk)
        @background_threads ||= []
        server, background_thread = start(cli_params, background: true, &blk)
        @background_threads << background_thread
        server
      end

      def start(cli_params, background: false, &blk)
        itsi_file = Itsi::Server::Config.config_file_path(cli_params[:config_file])
        Itsi.log_debug "Constructing server #{cli_params}: #{itsi_file}"
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

          Itsi.log_info "Starting Itsi..."
          server.start
          @running.delete(server)
          Signal.trap(:INT, previous_handler)
          server
        end
        background ? [server, Thread.new(&run)] : run[]
      rescue Exception => e # rubocop:disable Lint/RescueException
        Itsi.log_error e.message
      end

      def static(cli_params)
        start(cli_params.merge(static: true))
      end

      def stop
        return unless (pid = get_pid)

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

      def get_pid(warn = true)
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
        Itsi::Server::Config.test!({})
      end

      def init
        Config.write_default
      end

      def reload
        return unless (pid = get_pid)

        Process.kill(:HUP, pid)
      end

      def restart
        return unless (pid = get_pid)

        Process.kill(:USR1, pid)
      end

      def passfile(options, subcmd)
        filename = options[:passfile]
        unless filename || subcmd == "echo"
          puts "Error: passfile not set. Use --passfile option to provide a path to a file containing hashed credentials."
          puts "This file contains hashed credentials and should not be included in source control without additional protection."
          exit(1)
        end
        algorithm = options.fetch(:algorithm, "sha256")

        unless %w[sha256 sha512 bcrypt argon2 none].include?(algorithm)
          puts "Invalid algorithm"
          exit(1)
        end

        case subcmd
        when "add", "echo"
          Passfile.send(subcmd, filename, algorithm)
        when "remove", "list"
          Passfile.send(subcmd, filename)
        else
          puts "Valid subcommands are: add | remove | list"
          exit(0)
        end
      end

      def unique_path(dir, filename)
        base = File.basename(filename, ".*")
        ext  = File.extname(filename)
        candidate = File.join(dir, filename)
        return candidate unless File.exist?(candidate)

        i = 1
        loop do
          new_name = "#{base}_#{i}#{ext}"
          candidate = File.join(dir, new_name)
          return candidate unless File.exist?(candidate)

          i += 1
        end
      end

      def save_or_print(filename, content, options)
        if options[:save_dir]
          FileUtils.mkdir_p(options[:save_dir])
          path = unique_path(options[:save_dir], filename)
          File.write(path, content)
          puts "Written to #{path}"
        else
          puts content
        end
      end

      def secret(options)
        require "openssl"
        require "base64"

        puts "Enter algorithm (one of: HS256, HS384, HS512, RS256, RS384, RS512, PS256, PS384, PS512, ES256, ES384):"
        alg = $stdin.gets.chomp.upcase

        case alg
        when /^HS(\d+)$/
          bits = ::Regexp.last_match(1).to_i
          bytes = bits / 8
          key = SecureRandom.random_bytes(bytes)
          pem = Base64.strict_encode64(key)
          content = "=== HMAC #{bits}-bit Secret (base64) ===\n#{pem}\n"
          save_or_print("hmac_#{bits}_secret.txt", content, options)

        when /^RS/, /^PS/
          rsa = OpenSSL::PKey::RSA.new(2048)
          priv = rsa.to_pem
          pub  = rsa.public_key.to_pem
          save_or_print("rsa_private.pem", "=== RSA Private Key ===\n#{priv}", options)
          save_or_print("rsa_public.pem",  "=== RSA Public Key ===\n#{pub}", options)

        when "ES256", "ES384"
          curve = (alg == "ES256" ? "prime256v1" : "secp384r1")
          ec = OpenSSL::PKey::EC.new(curve)
          ec.generate_key
          priv = ec.to_pem
          pub_ec = ec.dup
          pub_ec.private_key = nil
          pub = pub_ec.to_pem
          save_or_print("ecdsa_private.pem", "=== ECDSA Private Key ===\n#{priv}", options)
          save_or_print("ecdsa_public.pem",  "=== ECDSA Public Key ===\n#{pub}", options)

        else
          warn "Unsupported algorithm: #{alg}"
          exit 1
        end
      end

      def add_worker
        return unless (pid = get_pid)

        Process.kill(:TTIN, pid)
      end

      def remove_worker
        return unless (pid = get_pid)

        Process.kill(:TTOU, pid)
      end

      def status
        return unless (pid = get_pid)

        Itsi.log_info("Itsi running on #{pid}")
        Process.kill(:USR2, pid)
      end

      def load_route_middleware_stack(cli_params)
        middleware, errors = Config.build_config(cli_params,
                                                 Itsi::Server::Config.config_file_path(cli_params[:config_file_path]))
        if errors.any?
          puts errors
          []
        else
          middleware["middleware_loader"][]
        end
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
