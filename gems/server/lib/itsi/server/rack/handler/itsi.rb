return unless defined?(::Rackup::Handler) || defined?(Rack::Handler)

module Rack
  module Handler
    module Itsi

      def self.run(app, options = {})
        rack_app = Rack::Builder.parse_file(options[:config])

        ::Itsi::Server.new(
          binds: ["#{options.fetch(:host, "127.0.0.1")}:#{options.fetch(:Port, 3001)}"],
          workers: options.fetch(:workers, 1),
          threads: options.fetch(:threads, 1),
          scheduler_class: "Itsi::Scheduler",
          app: ->{ app }
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
