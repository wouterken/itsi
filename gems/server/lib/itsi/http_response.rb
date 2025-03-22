# frozen_string_literal: true
require 'forwardable'
require "stringio"
require "socket"

module Itsi

  class HttpResponse

    def respond _body=nil, _status=200, _header=nil, status: _status, headers: _header, body: _body, hijack: false, &blk
      self.status = status

      if headers
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
        self.close unless hijack
      else
        self.close
      end
    end
  end
end
