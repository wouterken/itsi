# frozen_string_literal: true

require "stringio"
require "socket"
require "uri"
require_relative "http_request/response_status_shortcodes"

module Itsi
  class HttpRequest
    include Server::TypedHandlers::ParamParser
    include ResponseStatusShortcodes
    attr_accessor :hijacked

    EMPTY_IO = StringIO.new("").tap { |io| io.set_encoding(Encoding::ASCII_8BIT) }

    RACK_HEADER_MAP = StandardHeaders::ALL.map do |header|
      rack_form = \
        if header == "content-type"
          "CONTENT_TYPE"
        elsif header == "content-length"
          "CONTENT_LENGTH"
        else
          "HTTP_#{header.upcase.gsub(/-/, "_")}"
        end
      [header, rack_form]
    end.to_h

    RACK_HEADER_MAP.default_proc = proc { |_, key| "HTTP_#{key.upcase.gsub(/-/, "_")}" }

    HTTP_09 = "HTTP/0.9"
    HTTP_09_ARR = ["HTTP/0.9"].freeze
    HTTP_10 = "HTTP/1.0"
    HTTP_10_ARR = ["HTTP/1.0"].freeze
    HTTP_11 = "HTTP/1.1"
    HTTP_11_ARR = ["HTTP/1.1"].freeze
    HTTP_20 = "HTTP/2.0"
    HTTP_20_ARR = ["HTTP/2.0"].freeze
    HTTP_30 = "HTTP/3.0"
    HTTP_30_ARR = ["HTTP/3.0"].freeze

    def to_rack_env
      path = self.path
      host = self.host
      version = self.version
      env = RackEnvPool.checkout
      env["SCRIPT_NAME"] = script_name
      env["REQUEST_METHOD"] = request_method
      env["REQUEST_PATH"] = env["PATH_INFO"] = path
      env["QUERY_STRING"] = query_string
      env["REMOTE_ADDR"] = remote_addr
      env["SERVER_PORT"] = port.to_s
      env["HTTP_HOST"] = env["SERVER_NAME"] = host
      env["HTTP_VERSION"] = env["SERVER_PROTOCOL"] = version
      env["itsi.request"] = self
      env["itsi.response"] = response
      env["rack.version"] = \
        case version
        when HTTP_09 then HTTP_09_ARR
        when HTTP_10 then HTTP_10_ARR
        when HTTP_11 then HTTP_11_ARR
        when HTTP_20 then HTTP_20_ARR
        when HTTP_30 then HTTP_30_ARR
        end
      env["rack.url_scheme"] = scheme
      env["rack.input"] = build_input_io
      env["rack.hijack"] = method(:hijack)
      each_header do |k, v|
        env[case k
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
      env
    end

    def respond(
      _body = nil, _status = 200, _headers = nil, # rubocop:disable Lint/UnderscorePrefixedVariableName
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
        if as
          begin
            validate!(json, as: as)
          rescue ValidationError => e
            json = { type: "error", message: "Validation Error: #{e.message}" }
            status = 400
          end
        end
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
        response.hijack(server_sock.fileno)
        server_sock.sync = true
        app_sock.sync = true
        app_sock
      end
    end

    def body
      @body ||= build_input_io
    end

    def build_input_io
      case body_parts
      when nil then EMPTY_IO
      when String then StringIO.new(body_parts).tap { |io| io.set_encoding(Encoding::ASCII_8BIT) }
      when Array then File.open(body_parts.first, "rb").tap { |io| io.set_encoding(Encoding::ASCII_8BIT) }
      else body_parts
      end
    end

    def validate!(params, as: nil)
      as ? apply_schema!(params, as) : params
    end

    def params(schema = nil)
      params = if url_encoded?
                 URI.decode_www_form(build_input_io.read).to_h
               elsif json?
                 JSON.parse(build_input_io.read)
               elsif multipart?
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
      if block_given?
        yield validated
      else
        raise "#params must take a block for multipart requests" if multipart?

        validated

      end
    rescue ValidationError => e
      if response.json?
        respond(json: { error: e.message }, status: 400)
      else
        respond(e.message, 400)
      end
    rescue StandardError => e
      Itsi.log_error e.message
      puts e.backtrace

      # Unexpected error.
      # Don't reveal potential sensitive information to client.
      if response.json?
        respond(json: { error: "Internal Server Error" }, status: 500)
      else
        respond("Internal Server Error", 500)
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
