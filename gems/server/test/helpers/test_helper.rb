# frozen_string_literal: true

ENV["ITSI_LOG"] = "off"

require "minitest/reporters"
require "itsi/server"
require "itsi/scheduler"
require "socket"
require "net/http"
require "minitest/autorun"

Minitest::Reporters.use! Minitest::Reporters::SpecReporter.new

def free_bind(protocol = "http", unix_socket: false)
  if unix_socket
    socket_path = "/tmp/itsi_socket_#{Process.pid}_#{rand(1000)}.sock"
    UNIXServer.new(socket_path).close
    protocol == "https" ? "tls://#{socket_path}" : "unix://#{socket_path}"
  else
    server = TCPServer.new("0.0.0.0", 0)
    port = server.addr[1]
    server.close
    "#{protocol}://0.0.0.0:#{port}"
  end
end

def server(app: nil, protocol: "http", bind: free_bind(protocol), itsi_rb: nil, cleanup: true, timeout: 5, &blk)
  itsi_rb ||= lambda do
    # Inline Itsi.rb
    bind bind
    workers 1
    threads 1
    log_level :warn
    run app if app
  end

  cli_params = {}
  cli_params[:binds] = [bind] if bind

  sync = Queue.new
  cli_params[:hooks] ||= {}
  cli_params[:hooks]["after_start"] = lambda do
    sync.push(true)
  end

  Itsi::Server.start_in_background_thread(cli_params, &itsi_rb)

  sync.pop
  uri = URI(bind)
  # Timeout.timeout(timeout) do
  RequestContext.new(uri, self).instance_exec(uri, &blk)
  # end
rescue StandardError => e
  puts e
  # puts e.message
  # puts e.backtrace.join("\n")
  raise
ensure
  Itsi::Server.stop_background_threads if cleanup
end

require "net/http"
require "net_http_unix"
require "uri"

class RequestContext
  def initialize(uri, binding)
    @uri = uri
    @binding = binding
  end

  def method_missing(method_name, *args, &block)
    @binding.send(method_name, *args, &block)
  end

  def post(path, data = "", headers = {})
    client.post(uri_for(path), data, headers)
  end

  def get(path, headers = {})
    request = Net::HTTP::Get.new(uri_for(path))
    headers.each { |k, v| request[k] = v }
    client.request(request).body
  end

  def get_resp(path, headers = {})
    request = Net::HTTP::Get.new(uri_for(path))
    headers.each { |k, v| request[k] = v }
    client.request(request)
  end

  def head(path)
    request = Net::HTTP::Head.new(uri_for(path))
    client.request(request)
  end

  def options(path, headers = {})
    request = Net::HTTP::Options.new(uri_for(path))
    headers.each { |k, v| request[k] = v }
    client.request(request)
  end

  def put(path, data = "", headers = {})
    request = Net::HTTP::Put.new(uri_for(path))
    request.body = data
    headers.each { |k, v| request[k] = v }
    client.request(request)
  end

  def delete(path, headers = {})
    request = Net::HTTP::Delete.new(uri_for(path))
    headers.each { |k, v| request[k] = v }
    client.request(request)
  end

  def patch(path, data = "", headers = {})
    request = Net::HTTP::Patch.new(uri_for(path))
    request.body = data
    client.request(request)
  end

  private

  def client
    opts = {
      read_timeout: 1,
      open_timeout: 1
    }
    if @uri.scheme == "unix"
      NetX::HTTPUnix.new(
        @uri.to_s,
        **opts
      )
    else
      Net::HTTP.start(
        @uri.host,
        @uri.port,
        use_ssl: @uri.scheme == "https",
        **opts
      )
    end
  end

  def uri_for(path)
    if @uri.scheme == "unix"
      URI::HTTP.build(path: path, host: "localhost")
    else
      URI.join(@uri.to_s, path)
    end
  end
end
