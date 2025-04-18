module Itsi
  class Server
    module Config
      class RedirectHttpToHttps < Option

        insert_text <<~SNIPPET
        redirect_http_to_https! # Install a location block to redirect HTTP to HTTPS
                                # (Only works if listening on ports 443 and 80).
                                # For non-conventional or development ports use a manual redirect
        SNIPPET

        detail "Install a location block to redirect HTTP to HTTPS."

        def self.option_name
          :redirect_http_to_https!
        end

        def build!
          location.instance_eval do
            location protocols: [:http] do
              redirect \
                to: "https://{host}{path_and_query}", \
                type: "moved_permanently"
            end
          end
        end
      end
    end
  end
end
