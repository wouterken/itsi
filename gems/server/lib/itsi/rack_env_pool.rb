# frozen_string_literal: true

module Itsi
  module RackEnvPool

    RACK_ENV_TEMPLATE = {
      "SERVER_SOFTWARE" => "Itsi",
      "rack.errors" => $stderr,
      "rack.multithread" => true,
      "rack.multiprocess" => true,
      "rack.run_once" => false,
      "rack.hijack?" => true,
      "rack.multipart.buffer_size" => 16_384,
      "SCRIPT_NAME" => "",
      "REQUEST_METHOD" => "",
      "PATH_INFO" => "",
      "REQUEST_PATH" => "",
      "QUERY_STRING" => "",
      "REMOTE_ADDR" => "",
      "SERVER_PORT" => "",
      "SERVER_NAME" => "",
      "SERVER_PROTOCOL" => "",
      "HTTP_HOST" => "",
      "HTTP_VERSION" => "",
      "itsi.request" => "",
      "itsi.response" => "",
      "rack.version" => nil,
      "rack.url_scheme" => "",
      "rack.input" => "",
      "rack.hijack" => ""
    }.freeze

    POOL = []

    def self.checkout # rubocop:disable Metrics/CyclomaticComplexity,Metrics/MethodLength
      POOL.pop&.tap do |recycled|
        recycled.keys.each do |key|
          case key
          when "SERVER_SOFTWARE" then recycled[key] = "Itsi"
          when "rack.errors" then recycled[key] = $stderr
          when "rack.multithread", "rack.multiprocess", "rack.hijack?" then recycled[key] = true
          when "rack.run_once" then recycled[key] = false
          when "rack.multipart.buffer_size" then recycled[key] = 16_384
          when "SCRIPT_NAME", "REQUEST_METHOD", "PATH_INFO", "REQUEST_PATH", "QUERY_STRING", "REMOTE_ADDR",
               "SERVER_PORT", "SERVER_NAME", "SERVER_PROTOCOL", "HTTP_HOST", "HTTP_VERSION", "itsi.request",
               "itsi.response", "rack.version", "rack.url_scheme", "rack.input", "rack.hijack"
            nil
          else recycled.delete(key)
          end
        end
      end || RACK_ENV_TEMPLATE.dup
    end

    def self.checkin(env)
      POOL << env
    end
  end

end
