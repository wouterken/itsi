# frozen_string_literal: true

require "stringio"
require "socket"
require "uri"
require_relative 'http_request/response_status_shortcodes'

module Itsi
  class HttpRequest
    include Server::TypedHandlers::ParamParser
    include ResponseStatusShortcodes
    attr_accessor :hijacked

    EMPTY_IO = StringIO.new("")
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

    def respond(
      _body = nil, _status = 200, _headers = nil,
      json: nil,
      html: nil,
      text: nil,
      xml: nil,
      hijack: false,
      as: nil,
      status: _status,
      headers: _headers,
      body: _body,
      &blk
    )

      if json
        validate!(json, as: as) if as
        body = json.to_json
        headers ||= {}
        headers["Content-Type"] ||= "application/json"
      elsif html
        body = html
        headers ||= {}
        headers["Content-Type"] ||= "text/html"
      elsif xml
        body = xml
        headers ||= {}
        headers["Content-Type"] ||= "application/xml"
      elsif text
        body = text
        headers ||= {}
        headers["Content-Type"] ||= "text/plain"
      end

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

    def validate!(params, as: nil)
      as ? apply_schema!(params, as) : params
    end

    def params(schema=nil)
      params = case
      when url_encoded? then URI.decode_www_form(build_input_io.read).to_h
      when json? then JSON.parse(build_input_io.read)
      when multipart?
        Rack::Multipart::Parser.parse(
          build_input_io,
          content_length,
          content_type,
          Rack::Multipart::Parser::TEMPFILE_FACTORY,
          Rack::Multipart::Parser::BUFSIZE,
          Rack::Utils.default_query_parser
        ).params
      else
        {}
      end

      params.merge!(query_params).merge!(url_params)
      validated = schema ? apply_schema!(params, schema) : params
      unless block_given?
        if multipart?
          raise "#params must take a block for multipart requests"
        else
          return validated
        end
      else
        yield validated
      end

    rescue StandardError => e
      puts e.backtrace
      if response.json?
        respond(json: {error: e.message}, status: 400)
      else
        respond(e.message, 400)
      end
    ensure
      clean_temp_files(params)
    end

    def clean_temp_files(params)
      case params
      when Hash
        if params.key?(:tempfile)
          params[:tempfile].unlink
        else
        params.each_value { |v| clean_temp_files(v) }
        end
      when Array then params.each { |v| clean_temp_files(v) }
      end
    end

    def query_params
      URI.decode_www_form(query_string).to_h
    end
  end
end
