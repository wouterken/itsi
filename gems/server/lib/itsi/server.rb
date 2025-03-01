# frozen_string_literal: true

require_relative "server/version"
require_relative "server/itsi_server"
require_relative "request"

module Itsi
  class Server
    # Call our Rack app with our request ENV.
    def self.call(app, request)
      respond(app.call(request.to_env))
    end

    def self.respond((status, headers, body))
      [status, transform_headers(headers), body]
    end

    def self.transform_headers(headers)
      headers.map { |key, value| Array(value).map { |v| [key, v] } }.flatten(1)
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
