# frozen_string_literal: true

module Itsi
  class Server
    module Config
      require_relative "config/dsl"
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

        env = ENV.to_h.merge("OBJC_DISABLE_INITIALIZE_FORK_SAFETY" => "YES", "PGGSSENCMODE" => "YES")
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
        itsifile_config = \
          if builder_proc
            DSL.evaluate(&builder_proc)
          elsif args[:static]
            DSL.evaluate do
              location "/" do
                allow_list allowed_patterns: ['127.0.0.1']
                rate_limit key: 'address', store_config: 'in_memory', requests: 2, seconds: 5
                etag type: 'strong', algorithm: 'md5', min_body_size: 1024 * 1024
                compress min_size: 1024 * 1024, level: 'fastest', algorithms: %w[zstd gzip brotli deflate], mime_types: %w[all], compress_streams: true
                log_requests before: { level: "INFO", format: "[{request_id}] {method} {path_and_query} - {addr} " }, after: { level: "INFO", format: "[{request_id}] └─ {status} in {response_time}" }
                static_assets \
                  relative_path: true,
                  allowed_extensions: [],
                  root_dir: '.',
                  not_found_behavior: {error: 'not_found'},
                  auto_index: true,
                  try_html_extension: true,
                  max_file_size_in_memory: 1024 * 1024, # 1MB
                  max_files_in_memory: 1000,
                  file_check_interval: 1,
                  serve_dot_files: true,
                  headers: {
                    'Cache-Control' => 'public, max-age=86400',
                    'X-Content-Type-Options' => 'nosniff'
                  }
              end
            end
          elsif File.exist?(config_file_path.to_s)
            DSL.evaluate(config_file_path)
          elsif File.exist?("./config.ru")
            DSL.evaluate do
              preload true
              rackup_file args.fetch(:rackup_file, "./config.ru")
            end
          else
            DSL.evaluate{}
          end

        itsifile_config.transform_keys!(&:to_sym)

        # We'll preload while we load config, if enabled.
        middleware_loader = itsifile_config.fetch(:middleware_loader, -> {})
        preload = args.fetch(:preload) { itsifile_config.fetch(:preload, false) }

        case preload
        # If we preload everything, then we'll load middleware and default rack app ahead of time
        when true
          preloaded_middleware = middleware_loader.call
          middleware_loader = -> { preloaded_middleware }
        # If we're just preloading a specific gem group, we'll do that here too
        when Symbol
          Bundler.require(preload)
        end
        {
          workers: args.fetch(:workers) { itsifile_config.fetch(:workers, 1) },
          worker_memory_limit: args.fetch(:worker_memory_limit) { itsifile_config.fetch(:worker_memory_limit, nil) },
          silence: args.fetch(:silence) { itsifile_config.fetch(:silence, false) },
          shutdown_timeout: args.fetch(:shutdown_timeout) { itsifile_config.fetch(:shutdown_timeout, 5) },
          hooks: itsifile_config.fetch(:hooks, nil),
          preload: !!preload,
          notify_watchers: itsifile_config.fetch(:notify_watchers, nil),
          threads: args.fetch(:threads) { itsifile_config.fetch(:threads, 1) },
          script_name: args.fetch(:script_name) { itsifile_config.fetch(:script_name, "") },
          streamable_body: args.fetch(:streamable_body) { itsifile_config.fetch(:streamable_body, false) },
          multithreaded_reactor: args.fetch(:multithreaded_reactor) do
            itsifile_config.fetch(:multithreaded_reactor, nil)
          end,
          scheduler_class: args.fetch(:scheduler_class) { itsifile_config.fetch(:scheduler_class, nil) },
          oob_gc_responses_threshold: args.fetch(:oob_gc_responses_threshold) do
            itsifile_config.fetch(:oob_gc_responses_threshold, nil)
          end,
          log_level: args.fetch(:log_level) { itsifile_config.fetch(:log_level, nil) },
          log_format: args.fetch(:log_format) { itsifile_config.fetch(:log_format, nil) },
          log_target: args.fetch(:log_target) { itsifile_config.fetch(:log_target, nil) },
          binds: args.fetch(:binds) { itsifile_config.fetch(:binds, ["http://0.0.0.0:3000"]) },
          middleware_loader: middleware_loader,
          listeners: args.fetch(:listeners) { nil }
        }.transform_keys(&:to_s)
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
        config_file_path ||= \
          if File.exist?(ITSI_DEFAULT_CONFIG_FILE)
            ITSI_DEFAULT_CONFIG_FILE
          elsif File.exist?("config/#{ITSI_DEFAULT_CONFIG_FILE}")
            "config/#{ITSI_DEFAULT_CONFIG_FILE}"
          end
        # Options simply pass through unless we've specified a config file
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

        puts "Writing default configuration..."
        File.open(ITSI_DEFAULT_CONFIG_FILE, "w") do |file|
          file.write(IO.read("#{__dir__}/Itsi.rb"))
        end
      end
    end
  end
end
