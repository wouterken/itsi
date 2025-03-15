require "net/http"

class HomeController < ApplicationController
  def index
    render json: { message: "Hello, World!" }
  end

  def full_hijack
    io = request.env['rack.hijack'].call
    io.write("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n")
    10.times do |i|
      sleep 0.25
      io.write("Hello World\r\n")
    end
    io.close
  end

  def chunked_encoding
    io = request.env['rack.hijack'].call
    io.write("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\n\r\n")
    10.times do |i|
      chunk = "Hello World\r\n" * Random.rand(3..10)
      io.write("#{chunk.length.to_s(16)}\r\n#{chunk}\r\n")
      sleep 0.5
    end
    io.write("0\r\n\r\n")
    io.close
  end

  def io_party
    post = Post.find_or_create_by(name: "Hello World", body: "I made a change. This is a test post")
    ActiveRecord::Base.connection.execute("SELECT * FROM posts;")
    sleep 0.0001
    ActiveRecord::Base.connection.execute("SELECT pg_sleep(0.0001);")

    queue = Queue.new
    Thread.new do
      sleep 0.0001
      queue.push("done")
    end
    queue.pop

    Thread.new do
      sleep 0.0001
    end.join

    result = Net::HTTP.get(URI("https://www.cloudflare.com/cdn-cgi/trace"))
    post.update(name: "I made a change. Hello World", body: "Wow... I think it might be working")
    render json: post.to_json
  end
end
