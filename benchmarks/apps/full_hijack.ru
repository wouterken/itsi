run Proc.new { |env|
  if env['rack.hijack']
    io = env['rack.hijack'].call

    # Write raw HTTP/1.1 response headers
    io.write "HTTP/1.1 200 OK\r\n"
    io.write "Content-Type: text/plain\r\n"
    io.write "Connection: close\r\n"
    io.write "\r\n"

    # Write the response body
    io.write "Hello from full hijack!\n"

    io.close
  end

  [-1, {}, []]
}
