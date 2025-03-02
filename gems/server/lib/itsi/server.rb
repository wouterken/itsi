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

    def self.respond(response)
      status, headers, body = response
      [status, to_header_pairs(headers), body]
    end

    def self.to_header_pairs(headers)
      transformed = []
      headers.each do |key, value|
        if value.is_a?(Array)
          value.each do |v|
            transformed << key
            transformed << v
          end
        else
          transformed << key
          transformed << value
        end
      end
      transformed
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
