# frozen_string_literal: true
module Itsi
  class Request
    def to_env
      {
        "SERVER_SOFTWARE" => "Itsi",
        "SCRIPT_NAME" => script_name,
        "REQUEST_METHOD" => method,
        "PATH_INFO" => path,
        "REQUEST_PATH" => path,
        "QUERY_STRING" => query_string,
        "REMOTE_ADDR" => remote_addr,
        "SERVER_NAME" => host,
        "SERVER_PORT" => port.to_s,
        "SERVER_PROTOCOL" => version,
        "HTTP_VERSION" => version,
        "HTTP_HOST" => host,
        "rack.input" => StringIO.new(body),
        "rack.errors" => $stderr,
        "rack.version" => version,
        "rack.multithread" => true,
        "rack.multiprocess" => true,
        "rack.run_once" => false,
        "rack.multipart.buffer_size" => 16_384
      }.merge(
        headers.map do |k,v|
          [
            case k
            when "content-length" then "CONTENT_LENGTH"
            when "content-type" then "CONTENT_TYPE"
            else "HTTP_#{k.upcase.tr("-", "_")}"
            end,
            v
          ]
        end.to_h
      )
    end
  end
end
