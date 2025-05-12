# frozen_string_literal: true

module Itsi
  class Server
    module Config
      require_relative "config/typed_struct"
      require_relative "config/dsl"
      require_relative "config/known_paths"
      require_relative "default_app/default_app"

      ITSI_DEFAULT_CONFIG_FILE = "Itsi.rb"

      def self.prep_reexec!
        @argv ||= ARGV[0...ARGV.index("--listeners")]

        auto_suppress_fork_darwin_fork_safety_warnings = [
          ENV["OBJC_DISABLE_INITIALIZE_FORK_SAFETY"].nil? || ENV["PGGSSENCMODE"].nil?,
          RUBY_PLATFORM =~ /darwin/,
          !ENV.key?("ITSI_DISABLE_AUTO_DISABLE_DARWIN_FORK_SAFETY_WARNINGS"),
          $PROGRAM_NAME =~ /itsi$/
        ].all?
        return unless auto_suppress_fork_darwin_fork_safety_warnings

        env = ENV.to_h.merge("OBJC_DISABLE_INITIALIZE_FORK_SAFETY" => "YES", "PGGSSENCMODE" => "disable")
        if ENV["BUNDLE_BIN_PATH"]
          exec env, "bundle", "exec", $PROGRAM_NAME, *@argv
        else
          exec env, $PROGRAM_NAME, *@argv
        end
      end

      # The configuration used when launching the Itsi server are evaluated in the following precedence:
      # 1. CLI Args.
      # 2. Itsi.rb file.
      # 3. Default values.
      def self.build_config(args, config_file_path, builder_proc = nil)
        args.transform_keys!(&:to_sym)

        itsifile_config, errors = \
          if builder_proc
            DSL.evaluate(&builder_proc)
          elsif args[:static]
            DSL.evaluate do
              rate_limit key: "address", store_config: "in_memory", requests: 5, seconds: 10
              etag type: "strong", algorithm: "md5", min_body_size: 1024 * 1024
              compress min_size: 1024 * 1024, level: "fastest", algorithms: %w[zstd gzip br deflate],
                        mime_types: %w[all], compress_streams: true
              log_requests before: { level: "DEBUG", format: "[{request_id}] {method} {path_and_query} - {addr} " },
                            after: { level: "DEBUG",
                                    format: "[{request_id}] └─ {status} in {response_time}" }
              nodelay false
              static_assets \
                relative_path: true,
                allowed_extensions: [],
                root_dir: ".",
                not_found_behavior: { error: "not_found" },
                auto_index: true,
                try_html_extension: true,
                max_file_size_in_memory: 1024 * 1024, # 1MB
                max_files_in_memory: 1000,
                file_check_interval: 1,
                serve_hidden_files: false,
                headers: {
                  "X-Content-Type-Options" => "nosniff"
                }
            end
          elsif File.exist?(config_file_path.to_s)
            DSL.evaluate do
              include config_file_path.gsub(".rb", "")
              rackup_file args[:rackup_file], script_name: "/" if args.key?(:rackup_file)
            end
          elsif File.exist?("./config.ru")
            DSL.evaluate do
              preload true
              rackup_file args.fetch(:rackup_file, "./config.ru"), script_name: "/"
            end
          else
            DSL.evaluate do
              rackup_file args[:rackup_file], script_name: "/" if args.key?(:rackup_file)
            end
          end

        itsifile_config.transform_keys!(&:to_sym)

        # We'll preload while we load config, if enabled.
        middleware_loader = itsifile_config.fetch(:middleware_loader, -> {})
        preload = args.fetch(:preload) { itsifile_config.fetch(:preload, false) }

        case preload
        # If we preload everything, then we'll load middleware and default rack app ahead of time
        when true
          begin
            Itsi.log_debug("Preloading middleware and default rack app")
            preloaded_middleware = middleware_loader.call
            middleware_loader = -> { preloaded_middleware }
          rescue Exception => e # rubocop:disable Lint/RescueException
            errors << [e, e.backtrace[0]]
          end
        # If we're just preloading a specific gem group, we'll do that here too
        when Symbol
          Itsi.log_debug("Preloading gem group #{preload}")
          Bundler.require(preload)
        end

        if itsifile_config[:daemonize] && !@daemonized
          @daemonized = true
          Itsi.log_info("Itsi is running in the background. Writing pid to #{Itsi::Server::Config.pid_file_path}")
          Itsi.log_info("To stop Itsi, run 'itsi stop' from this directory.")
          Process.daemon(true, false)
          Server.write_pid
        end

        srv_config = {
          workers: args.fetch(:workers) { itsifile_config.fetch(:workers, 1) },
          worker_memory_limit: args.fetch(:worker_memory_limit) { itsifile_config.fetch(:worker_memory_limit, nil) },
          silence: args.fetch(:silence) { itsifile_config.fetch(:silence, false) },
          shutdown_timeout: args.fetch(:shutdown_timeout) { itsifile_config.fetch(:shutdown_timeout, 5) },
          hooks: if args[:hooks] && itsifile_config[:hooks]
                   args[:hooks].merge(itsifile_config[:hooks])
                 else
                   itsifile_config.fetch(
                     :hooks, args[:hooks]
                   )
                 end,
          preload: !!preload,
          request_timeout: itsifile_config.fetch(:request_timeout, nil),
          header_read_timeout: args.fetch(:header_read_timeout) { itsifile_config.fetch(:header_read_timeout, nil) },
          notify_watchers: itsifile_config.fetch(:notify_watchers, nil),
          threads: args.fetch(:threads) { itsifile_config.fetch(:threads, 1) },
          scheduler_threads: args.fetch(:scheduler_threads) { itsifile_config.fetch(:scheduler_threads, nil) },
          streamable_body: args.fetch(:streamable_body) { itsifile_config.fetch(:streamable_body, false) },
          multithreaded_reactor: args.fetch(:multithreaded_reactor) do
            itsifile_config.fetch(:multithreaded_reactor, nil)
          end,
          pin_worker_cores: args.fetch(:pin_worker_cores) { itsifile_config.fetch(:pin_worker_cores, true) },
          scheduler_class: args.fetch(:scheduler_class) { itsifile_config.fetch(:scheduler_class, nil) },
          oob_gc_responses_threshold: args.fetch(:oob_gc_responses_threshold) do
            itsifile_config.fetch(:oob_gc_responses_threshold, nil)
          end,
          ruby_thread_request_backlog_size: args.fetch(:ruby_thread_request_backlog_size) do
            itsifile_config.fetch(:ruby_thread_request_backlog_size, nil)
          end,
          log_level: args.fetch(:log_level) { itsifile_config.fetch(:log_level, nil) },
          log_format: args.fetch(:log_format) { itsifile_config.fetch(:log_format, nil) },
          log_target: args.fetch(:log_target) { itsifile_config.fetch(:log_target, nil) },
          log_target_filters: args.fetch(:log_target_filters) { itsifile_config.fetch(:log_target_filters, nil) },
          binds: args.fetch(:binds) { itsifile_config.fetch(:binds, ["http://0.0.0.0:3000"]) },
          middleware_loader: middleware_loader,
          listeners: args.fetch(:listeners) { nil },
          reuse_address: itsifile_config.fetch(:reuse_address, true),
          reuse_port: itsifile_config.fetch(:reuse_port, true),
          listen_backlog: itsifile_config.fetch(:listen_backlog, 1024),
          nodelay: itsifile_config.fetch(:nodelay, true),
          recv_buffer_size: itsifile_config.fetch(:recv_buffer_size, 262_144),
          send_buffer_size: itsifile_config.fetch(:send_buffer_size, 262_144)
        }.transform_keys(&:to_s)

        [srv_config, errors_to_error_lines(errors)]
      rescue StandardError => e
        [{}, errors_to_error_lines([[e, e.backtrace[0]]])]
      end

      def self.test!(cli_params)
        config, errors = build_config(cli_params, Itsi::Server::Config.config_file_path(cli_params[:config_file]))
        unless errors.any?
          begin
            config["middleware_loader"][]
          rescue Exception => e # rubocop:disable Lint/RescueException
            errors = [e]
          end
        end

        if errors.any?
          Itsi.log_error("Config file is invalid")
          puts errors
        else
          Itsi.log_info("Config file is valid")
        end
      end

      def self.errors_to_error_lines(errors)
        return unless errors

        errors.flat_map do |(error, message)|
          location = message[/(.*?):in/, 1]
          file, lineno = location.split(":")
          lineno = lineno.to_i
          err_message = error.is_a?(NoMethodError) && error.respond_to?(:detailed_message) ? error.detailed_message : error.message
          file_lines = IO.readlines(file)
          info_lines = \
            if error.is_a?(SyntaxError)
              []
            else

              ([lineno - 2, 0].max...[file_lines.length, lineno.succ.succ].min).map do |currline|
                if currline == lineno - 1
                  line = file_lines[currline][0...-1]
                  padding = line[/^\s+/]&.length || 0

                  [
                    " \e[31m#{currline.succ.to_s.rjust(3)} | #{line}\e[0m",
                    "     | #{" " * padding}\e[33m^^^\e[0m "
                  ]
                else
                  " #{currline.succ.to_s.rjust(3)} | #{file_lines[currline][0...-1]}"
                end
              end.flatten
            end
          [
            err_message,
            "   --> #{File.expand_path(file)}:#{lineno}",
            *info_lines
          ]
        end
      end

      # Reloads the entire process
      # using exec, passing in any active file descriptors
      # and previous invocation arguments
      def self.reload_exec(listener_info)
        if ENV["BUNDLE_BIN_PATH"]
          exec "bundle", "exec", $PROGRAM_NAME, *@argv, "--listeners", listener_info
        else
          exec $PROGRAM_NAME, *@argv, "--listeners", listener_info
        end
      end

      # Find config file path, if it exists.
      def self.config_file_path(config_file_path = nil)

        if config_file_path && !File.exist?(config_file_path)
          raise "Config file #{config_file_path} does not exist"
        end

        config_file_path ||= \
          if File.exist?(ITSI_DEFAULT_CONFIG_FILE)
            ITSI_DEFAULT_CONFIG_FILE
          elsif File.exist?("config/#{ITSI_DEFAULT_CONFIG_FILE}")
            "config/#{ITSI_DEFAULT_CONFIG_FILE}"
          end
        # Options pass through unless we've specified a config file
        return unless File.exist?(config_file_path.to_s)

        config_file_path
      end

      def self.pid_file_path
        if Dir.exist?("tmp")
          File.join("tmp", "itsi.pid")
        else
          ".itsi.pid"
        end
      end

      # Write a default config file, if one doesn't exist.
      def self.write_default
        if File.exist?(ITSI_DEFAULT_CONFIG_FILE)
          puts "#{ITSI_DEFAULT_CONFIG_FILE} already exists."
          return
        end

        default_config = IO.read("#{__dir__}/default_config/Itsi.rb")

        default_config << \
          if File.exist?("./config.ru")
            <<~RUBY
              # You can mount several Ruby apps as either
              # 1. rackup files
              # 2. inline rack apps
              # 3. inline Ruby endpoints
              #
              # 1. rackup_file
              rackup_file "./config.ru"
              #
              # 2. inline rack app
              # require 'rack'
              # run(Rack::Builder.app do
              #   use Rack::CommonLogger
              #   run ->(env) { [200, { 'content-type' => 'text/plain' }, ['OK']] }
              # end)
              #
              # 3. Endpoints
              # endpoint "/" do |req|
              #   req.ok "Hello from Itsi"
              # end
            RUBY
          else
            <<~RUBY
              # You can mount several Ruby apps as either
              # 1. rackup files
              # 2. inline rack apps
              # 3. inline Ruby endpoints
              #
              # 1. rackup_file
              # Use `rackup_file` to specify the Rack app file name.
              #
              # 2. inline rack app
              # require 'rack'
              # run(Rack::Builder.app do
              #   use Rack::CommonLogger
              #   run ->(env) { [200, { 'content-type' => 'text/plain' }, ['OK']] }
              # end)
              #
              # 3. Endpoint
              endpoint "/" do |req|
                req.ok "Hello from Itsi"
              end
            RUBY
          end

        File.open(ITSI_DEFAULT_CONFIG_FILE, "w") do |file|
          file.write(default_config)
        end
      end
    end
  end
end
