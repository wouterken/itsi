# frozen_string_literal: true

module Itsi
  class Request
    require "stringio"
    require "socket"

    attr_accessor :hijacked

    def to_env
      path = self.path
      host = self.host
      version = self.version
      body = self.body
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
        "rack.version" => [version],
        "rack.url_scheme" => scheme,
        "rack.input" => \
          case body
          when Array then File.open(body.first, "rb")
          when String then StringIO.new(body)
          else body
          end,
        "rack.errors" => $stderr,
        "rack.multithread" => true,
        "rack.multiprocess" => true,
        "rack.run_once" => false,
        "rack.hijack?" => true,
        "rack.multipart.buffer_size" => 16_384,
        "rack.hijack" => lambda do
          self.hijacked = true
          UNIXSocket.pair.yield_self do |(server_sock, app_sock)|
            response.hijack(server_sock.fileno)
            server_sock.sync = true
            app_sock.sync = true
            app_sock.instance_variable_set("@server_sock", server_sock)
            app_sock
          end
        end
      }.tap { |r| headers.each { |(k, v)| r[k] = v } }
    end
  end
end
