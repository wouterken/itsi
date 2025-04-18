module Itsi
  class Server
    module Config
      class AutoReloadConfig < Option

        insert_text <<~SNIPPET
        auto_reload_config! # Auto-reload the server configuration each time it changes.
        SNIPPET

        detail "Auto-reload the server configuration each time it changes."

        def self.option_name
          :auto_reload_config!
        end

        def build!
          location.instance_eval do
            return if @auto_reloading

            @auto_reloading = true

            if ENV["BUNDLE_BIN_PATH"]
              watch "Itsi.rb", [%w[bundle exec itsi restart]]
            else
              watch "Itsi.rb", [%w[itsi restart]]
            end
          end
        end
      end
    end
  end
end
