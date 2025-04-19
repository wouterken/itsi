module Itsi
  class Server
    module Config
      class Run < Middleware

        insert_text <<~SNIPPET
        run \\
          ${1:->(env){[200, {"Content-Type" => "text/plain"\\}, ["Hello, World!"]]}},
          nonblocking: ${2|true,false|},
          sendfile: ${3|true,false|}
        SNIPPET

        detail "Define an inline Rack Application"

        schema do
          {
            nonblocking: Bool().default(false),
            sendfile: Bool().default(true)
          }
        end

        def initialize(location, app, params={})
          super(location, params)
          raise "App must be a Rack application" unless app.respond_to?(:call)
          @app = app
        end

        def build!
          app_args = {
            preloader: -> { Itsi::Server::RackInterface.for(@app) },
            sendfile: @params[:sendfile],
            nonblocking: @params[:nonblocking],
            base_path: "^(?<base_path>#{location.paths_from_parent.gsub(/\.\*\)$/, ')')}).*$"
          }
          location.middleware[:app] = app_args
          location.location("*") do
            @middleware[:app] = app_args
          end
        end
      end
    end
  end
end
