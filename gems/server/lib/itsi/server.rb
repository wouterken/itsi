# frozen_string_literal: true

require_relative "server/version"
require_relative "server/itsi_server"

module Itsi
  class Server
    # Call our Rack app with our request ENV.
    def self.call(app, request)
      app.call(request.to_env)
    end

    # If scheduler is enabled
    # Each request is wrapped in a Fiber.
    def self.schedule(app, request)
      Fiber.schedule do
        call(app, request)
      end
    end
  end
end
