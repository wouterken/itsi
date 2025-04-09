# frozen_string_literal: true
# ENV["ITSI_LOG"] = "off"

require "minitest/reporters"
require "itsi/server"
require "itsi/scheduler"
require "socket"
require "net/http"
require "minitest/autorun"


Minitest::Reporters.use! Minitest::Reporters::SpecReporter.new

def free_bind(protocol)
  server = TCPServer.new("0.0.0.0", 0)
  port = server.addr[1]
  server.close
  "#{protocol}://0.0.0.0:#{port}"
end

def server(app: nil, protocol: "http", bind: free_bind(protocol), itsi_rb: nil, &blk)
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

  Itsi::Server.start_in_background_thread(cli_params, &itsi_rb)
  uri = URI(bind)
  RequestContext.new(uri, self).instance_exec(uri, &blk)
rescue StandardError => e
  puts e
  puts e.message
  puts e.backtrace.join("\n")
ensure
  Itsi::Server.stop_background_thread
end

class RequestContext
  def initialize(uri, binding)
    @uri = uri
    @binding = binding
  end

  def method_missing(method_name, *args, &block)
    @binding.send(method_name, *args, &block)
  end

  def post(path, data)
    Net::HTTP.post(@uri+path, data)
  end

  def get(path)
    Net::HTTP.get(@uri+path)
  end

  def get_resp(path)
    Net::HTTP.get_response(@uri+path)
  end

  def head(path)
    Net::HTTP.start(@uri.host, @uri.port) {|http|
      http.head(path)
    }
  end

  def options(path)
    Net::HTTP.start(@uri.host, @uri.port) {|http|
      http.options(path)
    }
  end

  def put(path, data)
    Net::HTTP.put(@uri+path, data)
  end

  def delete(path)
    Net::HTTP.delete(@uri+path)
  end

  def patch(path, data)
    Net::HTTP.patch(@uri+path, data)
  end
end
