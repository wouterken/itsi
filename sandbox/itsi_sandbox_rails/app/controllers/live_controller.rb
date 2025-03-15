require "net/http"
require 'debug'
class LiveController < ApplicationController
  include ActionController::Live

  def stream
    response.headers['Content-Type'] = 'text/event-stream'
    response.headers["Last-Modified"] = Time.now.httpdate
    i = 0
    loop {
      i += 1
      response.stream.write "hello world #{i}\r\n"
      sleep 0.00005
    }

  rescue ActionController::Live::ClientDisconnected => e
    puts "Client disconnected"

  ensure
    puts "Closing stream"
    response.stream.close
  end

  def sse
    response.headers['Content-Type'] = 'text/event-stream'
    response.headers["Last-Modified"] = Time.now.httpdate
    sse = SSE.new(response.stream, retry: 300, event: "event-name")
    loop do
      sse.write({ name: 'John'})
      sse.write({ name: 'John'}, id: 10)
      sse.write({ name: 'John'}, id: 10, event: "other-event")
      sse.write({ name: 'John'}, id: 10, event: "other-event", retry: 500)
      sleep 0.00005
    end
  rescue ActionController::Live::ClientDisconnected => e
    puts "Client disconnected"
  ensure
    puts "Closing stream"
    sse.close
  end
end
