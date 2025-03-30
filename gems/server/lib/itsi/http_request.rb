# frozen_string_literal: true

require "stringio"
require "socket"

module Itsi
  class HttpRequest
    attr_accessor :hijacked

    EMPTY_IO = StringIO.new("").freeze
    RACK_HEADER_MAP = StandardHeaders::ALL.map do |header|
      rack_form = if header == "content-type"
                    "CONTENT_TYPE"
                  elsif header == "content-length"
                    "CONTENT_LENGTH"
                  else
                    "HTTP_#{header.upcase.gsub(/-/, "_")}"
                  end
      [header, rack_form]
    end.to_h.tap do |hm|
      hm.default_proc = proc { |hsh, key| "HTTP_#{key.upcase.gsub(/-/, "_")}" }
    end

    def to_rack_env
      path = self.path
      host = self.host
      version = self.version
      {
        "SERVER_SOFTWARE" => "Itsi",
        "SCRIPT_NAME" => script_name,
        "REQUEST_METHOD" => request_method,
        "PATH_INFO" => path,
        "REQUEST_PATH" => path,
        "QUERY_STRING" => query_string,
        "REMOTE_ADDR" => remote_addr,
        "SERVER_PORT" => port.to_s,
        "SERVER_NAME" => host,
        "SERVER_PROTOCOL" => version,
        "HTTP_HOST" => host,
        "HTTP_VERSION" => version,
        "itsi.request" => self,
        "itsi.response" => response,
        "rack.version" => [version],
        "rack.url_scheme" => scheme,
        "rack.input" => build_input_io,
        "rack.errors" => $stderr,
        "rack.multithread" => true,
        "rack.multiprocess" => true,
        "rack.run_once" => false,
        "rack.hijack?" => true,
        "rack.multipart.buffer_size" => 16_384,
        "rack.hijack" => method(:hijack)
      }.tap do |r|
        headers.each do |(k, v)|
          r[case k
            when "content-type" then "CONTENT_TYPE"
            when "content-length" then "CONTENT_LENGTH"
            when "accept" then "HTTP_ACCEPT"
            when "accept-encoding" then "HTTP_ACCEPT_ENCODING"
            when "accept-language" then "HTTP_ACCEPT_LANGUAGE"
            when "user-agent" then "HTTP_USER_AGENT"
            when "referer" then "HTTP_REFERER"
            when "origin" then "HTTP_ORIGIN"
            when "cookie" then "HTTP_COOKIE"
            when "authorization" then "HTTP_AUTHORIZATION"
            when "x-forwarded-for" then "HTTP_X_FORWARDED_FOR"
            when "x-forwarded-proto" then "HTTP_X_FORWARDED_PROTO"
            else RACK_HEADER_MAP[k]
            end
          ] = v
        end
      end
    end

    def respond(_body = nil, _status = 200, _header = nil, status: _status, headers: _header, body: _body,
                hijack: false, &blk)
      response.respond(status: status, headers: headers, body: body, hijack: hijack, &blk)
    end

    def hijack
      self.hijacked = true
      UNIXSocket.pair.yield_self do |(server_sock, app_sock)|
        server_sock.autoclose = false
        self.response.hijack(server_sock.fileno)
        server_sock.sync = true
        app_sock.sync = true
        app_sock
      end
    end

    def build_input_io
      case body
      when nil then EMPTY_IO
      when String then StringIO.new(body)
      when Array then File.open(body.first, "rb")
      else body
      end
    end
  end
end
