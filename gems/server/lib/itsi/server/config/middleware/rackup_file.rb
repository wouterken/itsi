module Itsi
  class Server
    module Config
      class RackupFile < Middleware
        insert_text <<~SNIPPET
          rackup_file \\
            "config.ru",
            nonblocking: ${2|true,false|},
            sendfile: ${3|true,false|}

        SNIPPET

        detail "Define an inline Rack Application"

        schema do
          {
            nonblocking: Bool().default(false),
            script_name: Type(String).default(nil),
            sendfile: Bool().default(true)
          }
        end

        def initialize(location, app, **params)
          super(location, params)
          raise "Rackup file must be a string" unless app.is_a?(String)

          @app = Itsi::Server::RackInterface.for(app)
        end

        def build!
          app_args = {
            preloader: -> { @app },
            sendfile: @params[:sendfile],
            nonblocking: @params[:nonblocking],
            script_name: @params[:script_name],
            base_path: "^(?<base_path>#{location.paths_from_parent.gsub(/\.\*\)$/, ")")}).*$"
          }
          location.middleware[:app] = app_args
        end
      end
    end
  end
end
