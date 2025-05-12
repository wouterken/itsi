return unless defined?(::Rackup::Handler) || defined?(Rack::Handler)

module Rack
  module Handler
    module Itsi
      def self.run(app, options = {})
        host = options.fetch(:host, "127.0.0.1")
        port = options.fetch(:Port, 3001)
        ::Itsi::Server.start(
          {
            binds: ["http://#{host}:#{port}"],
            threads: 5
          }
        ) do
          run app
        end
      end
    end
  end
end

if defined?(Rackup)
  ::Rackup::Handler.register("itsi", Rack::Handler::Itsi)
  ::Rackup::Handler.register("Itsi", Rack::Handler::Itsi)
elsif defined?(Rack)
  ::Rack::Handler.register("itsi", Rack::Handler::Itsi)
  ::Rack::Handler.register("Itsi", Rack::Handler::Itsi)
end
