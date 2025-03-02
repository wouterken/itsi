# frozen_string_literal: true

module Itsi
  class Request
    require 'stringio'
    def to_env
      path = self.path
      host = self.host
      version = self.version
      {
        "SERVER_SOFTWARE" => "Itsi",
        "SCRIPT_NAME" => script_name,
        "REQUEST_METHOD" => method,
        "PATH_INFO" => path,
        "REQUEST_PATH" => path,
        "QUERY_STRING" => query_string,
        "REMOTE_ADDR" => remote_addr,
        "SERVER_PORT" => port.to_s,
        "SERVER_NAME" => host,
        "HTTP_HOST" => host,
        "SERVER_PROTOCOL" => version,
        "HTTP_VERSION" => version,
        "rack.version" => version,
        "rack.url_scheme" => scheme,
        "rack.input" => StringIO.new(body),
        "rack.errors" => $stderr,
        "rack.multithread" => true,
        "rack.multiprocess" => true,
        "rack.run_once" => false,
        "rack.multipart.buffer_size" => 16_384
      }.merge(headers)
    end
  end
end
