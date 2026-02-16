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

    def self.checkout
      recycled = POOL.pop
      return RACK_ENV_TEMPLATE.dup unless recycled

      # Reset in C rather than iterating key-by-key in Ruby for every request.
      recycled.replace(RACK_ENV_TEMPLATE)
      recycled
    end

    def self.checkin(env)
      POOL << env
    end
  end

end
