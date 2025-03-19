module Itsi
  class Server
    module Config
      module_function

      ITSI_DEFAULT_CONFIG_FILE = "Itsi.rb"

      # Hunt for the config file, either at options[:config_file] or Itsi.rb or config/Itsi.rb
      # If found, we'll interpret it using the OptionsDSL.
      def load(config_file_path = "")
        config_file_path ||= \
          if File.exist?(ITSI_DEFAULT_CONFIG_FILE)
            ITSI_DEFAULT_CONFIG_FILE
          elsif File.exist?("config/#{ITSI_DEFAULT_CONFIG_FILE}")
            "config/#{ITSI_DEFAULT_CONFIG_FILE}"
          end

        # Options simply pass through unless we've specified a config file
        return {} unless config_file_path && File.exist?(config_file)

        require_relative "options_dsl"
        OptionsDSL.evaluate(config_file_path)
      end

      # Write the default Itsi.rb file (found relative to this file at ./Itsi.rb)
      # This is invoked when you call `itsi init` from the CLI.
      def write_default
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
