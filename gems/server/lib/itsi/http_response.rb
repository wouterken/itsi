# frozen_string_literal: true

require "forwardable"
require "stringio"
require "socket"

module Itsi
  class HttpResponse
    def respond(
      _body = nil, _status = 200, _header = nil, # rubocop:disable Lint/UnderscorePrefixedVariableName
      status: _status, headers: _header, body: _body,
      hijack: false
    )
      self.status = status.is_a?(Symbol) ? HTTP_STATUS_NAME_TO_CODE_MAP.fetch(status) : status.to_i

      body = body.to_s unless body.is_a?(String)

      if headers
        reserve_headers(headers.size)
        headers.each do |key, value|
          if value.is_a?(Array)
            value.each { |v| add_header(key, v) }
          else
            add_header(key, value)
          end
        end
      end

      if body
        # Common case. Write a single string body.
        send_and_close(body)
      elsif block_given?

        # If you call respond with a block, you get a handle to a stream that you can write to.
        yield self

        # If you hijack the connection, you are responsible for closing it.
        # Otherwise, the response will be closed automatically.
        close unless hijack
      else
        close
      end
    end
  end
end
