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
          return if @auto_reloading
          src = caller.find{|l| !(l =~ /lib\/itsi\/server\/config/) }.split(":").first

          location.instance_eval do
            return if @auto_reloading

            if @included
              @included.each do |file|
                next if  "#{file}.rb" == src
                if ENV["BUNDLE_BIN_PATH"]
                  watch "#{file}.rb", [%w[bundle exec itsi restart]]
                else
                  watch "#{file}.rb", [%w[itsi restart]]
                end
              end
            end
            @auto_reloading = true

            if ENV["BUNDLE_BIN_PATH"]
              watch src, [%w[bundle exec itsi restart]]
            else
              watch src, [%w[itsi restart]]
            end
          end
        end
      end
    end
  end
end
