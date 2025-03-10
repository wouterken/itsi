return unless defined?(::Rackup::Handler) || defined?(Rack::Handler)

module Rack
  module Handler
    module Itsi

      def self.run(app, options = {})
        ::Itsi::Server.new(
          app: ->{ app },
          binds: ["#{options.fetch(:host, "127.0.0.1")}:#{options.fetch(:Port, 3001)}"],
          workers: options.fetch(:workers, 1),
          threads: options.fetch(:threads, 1),
        ).start
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
