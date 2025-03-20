module Itsi
  class Server
    module Config
      require_relative "config/dsl"
      require_relative "default_app/default_app"
      require "etc"
      require "debug"

      ITSI_DEFAULT_CONFIG_FILE = "Itsi.rb"

      # The configuration used when launching the Itsi server are evaluated in the following precedence:
      # 1. CLI Args.
      # 2. Itsi.rb file.
      # 3. Default values.
      def self.build_config(args, config_file_path)
        itsifile_config = File.exist?(config_file_path.to_s) ? DSL.evaluate(config_file_path) : {}
        args.transform_keys!(&:to_sym)
        srv_config = {
          workers: args.fetch(:workers) { itsifile_config.fetch(:workers, Etc.nprocessors) },
          worker_memory_limit: args.fetch(:worker_memory_limit) { itsifile_config.fetch(:worker_memory_limit, nil) },
          silence: args.fetch(:silence) { itsifile_config.fetch(:silence, false) },
          shutdown_timeout: args.fetch(:shutdown_timeout) { itsifile_config.fetch(:shutdown_timeout, 5) },
          hooks: itsifile_config.fetch(:hooks, nil),
          preload: args.fetch(:preload) { itsifile_config.fetch(:preload, false) },
          threads: args.fetch(:threads) { itsifile_config.fetch(:threads, 1) },
          script_name: args.fetch(:script_name) { itsifile_config.fetch(:script_name, "") },
          streamable_body: args.fetch(:streamable_body) { itsifile_config.fetch(:streamable_body, false) },
          scheduler_class: args.fetch(:scheduler_class) { itsifile_config.fetch(:scheduler_class, nil) },
          oob_gc_responses_threshold: args.fetch(:oob_gc_responses_threshold) do
            itsifile_config.fetch(:oob_gc_responses_threshold, nil)
          end,
          binds: args.fetch(:binds) { itsifile_config.fetch(:binds, ["http://0.0.0.0:3000"]) },
          middleware_loader: itsifile_config.fetch(:middleware_loader, ->{}),
          default_app: { "rackup_loader" => itsifile_config.fetch(:rackup_loader, DEFAULT_APP) }
        }.transform_keys(&:to_s)
        puts #{srv_config}
        srv_config
      end

      # Reloads the entire process
      # using exec, passing in any active file descriptors
      # and previous invocation arguments
      def self.reload_exec(cli_args, listener_info)
        require "json"
        fork_params = { cli_args: cli_args, listener_info: listener_info }.to_json

        if ENV["BUNDLE_BIN_PATH"]
          # Launched via "bundle exec", so reapply bundler in your exec call.
          exec "bundle", "exec", $PROGRAM_NAME, "--reexec", fork_params
        else
          # Launched directly.
          exec $PROGRAM_NAME, "--reexec", fork_params
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
        return unless File.exist?(config_file_path)

        config_file_path
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
