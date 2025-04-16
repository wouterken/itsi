module Itsi
  class Server
    module Config
      class AutoReloadConfig < Option

        insert_text <<~SNIPPET
        auto_reload_config!
        SNIPPET

        detail "Auto-reload the server configuration each time it changes."

        def self.option_name
          :auto_reload_config!
        end

      end
    end
  end
end
