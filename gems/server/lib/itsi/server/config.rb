module Itsi
  class Server
    module Config
      module_function

      ITSI_DEFAULT_CONFIG_FILE = "Itsi.rb"

      def load(options)
        options[:config_file] ||= \
          if File.exist?(ITSI_DEFAULT_CONFIG_FILE)
            ITSI_DEFAULT_CONFIG_FILE
          elsif File.exist?("config/#{ITSI_DEFAULT_CONFIG_FILE}")
            "config/#{ITSI_DEFAULT_CONFIG_FILE}"
          end

        # Options simply pass through unless we've specified a config file
        return options unless options[:config_file]

        require_relative "options_dsl"
        OptionsDSL.evaluate(options[:config_file]).merge(options)
      end

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
