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

    PER_THREAD_KEY = :itsi_env_pool

    def self.checkout
      pool = Thread.current[PER_THREAD_KEY] ||= []
      pool.pop&.tap do |popped|
        popped.clear.merge(RACK_ENV_TEMPLATE)
      end || RACK_ENV_TEMPLATE.dup
    end

    def self.checkin(env)
      Thread.current[PER_THREAD_KEY] << env
    end
  end

end
