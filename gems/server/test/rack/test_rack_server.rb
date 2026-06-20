require_relative "../helpers/test_helper"
require "async/websocket/adapters/rack"
require "protocol/websocket/connection"
require "base64"

class TestRackServer < Minitest::Test
  def test_that_it_has_a_version_number
    refute_nil ::Itsi::Server::VERSION
  end

  def test_hello_world
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "text/plain" }, ["Hello, World!"]]
    end) do
      assert_equal "Hello, World!", get("/")
    end
  end

  def test_post
    server(app_with_lint: lambda do |env|
      assert_equal env["REQUEST_METHOD"], "POST"
      assert_equal "data", env["rack.input"].read
      [200, { "content-type" => "text/plain" }, ["Hello, World!"]]
    end) do
      assert_equal "Hello, World!", post("/", "data").body
    end
  end

  def test_full_hijack
    server(app_with_lint: lambda do |env|
      io = env["rack.hijack"].call
      io.write("HTTP/1.1 200 Ok\r\n")
      io.write("Content-Type: text/plain\r\n")
      io.write("Transfer-Encoding: chunked\r\n")
      io.write("\r\n")
      io.write("7\r\n")
      io.write("Hello, \r\n")
      io.write("6\r\n")
      io.write("World!\r\n")
      io.write("0\r\n\r\n")
      io.close
      [200, {}, []]
    end) do
      assert_equal "Hello, World!", get("/")
    end
  end

  def test_streaming_body
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "text/plain" }, lambda { |stream|
        stream.write("Hello")
        stream.write(", World!")
        stream.close
      }]
    end) do
      assert_equal "Hello, World!", get("/")
    end
  end

  def test_partial_hijack
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "text/plain", "rack.hijack" => lambda { |stream|
        stream.write("Hello")
        stream.write(", World!")
        stream.close
      } }, []]
    end) do
      assert_equal "Hello, World!", get("/")
    end
  end

  def test_partial_hijack_yields_writable_stream
    stream_class = nil
    stream_closed = nil
    required_methods_ok = nil

    server(app_with_lint: lambda do |_env|
      [200, { "content-type" => "text/plain", "rack.hijack" => lambda { |stream|
        stream_class = stream.class
        refute stream.closed?
        required_methods_ok = %i[read write << flush close close_read close_write closed?]
          .all? { |method_name| stream.respond_to?(method_name) }
        stream.write("Hello")
        stream.write(", World!")
        stream.close
        stream_closed = stream.closed?
      } }, []]
    end) do
      assert_equal "Hello, World!", get("/")
    end

    assert_equal Rack::Lint::Wrapper::StreamWrapper, stream_class
    assert_equal true, required_methods_ok
    assert_equal true, stream_closed
  end

  def test_partial_hijack_upgrade_supports_bidirectional_io
    callback_error = Queue.new
    stream_details = Queue.new

    server(app: lambda do |_env|
      [101, {
        "connection" => "Upgrade",
        "upgrade" => "test",
        "rack.hijack" => lambda { |stream|
          stream_details << [stream.class, stream.is_a?(IO), stream.method(:read).arity]
          Thread.new do
            begin
              payload = stream.read(2)
              stream.write("echo:#{payload}")
            rescue => e
              callback_error << e
            ensure
              stream.close rescue nil
            end
          end
        }
      }, []]
    end) do |uri|
      socket = TCPSocket.new(uri.host, uri.port)
      socket.write(
        "GET / HTTP/1.1\r\n" \
        "Host: #{uri.host}:#{uri.port}\r\n" \
        "Connection: Upgrade\r\n" \
        "Upgrade: test\r\n" \
        "\r\n"
      )
      socket.flush

      response = +""
      until response.include?("\r\n\r\n")
        response << socket.readpartial(1024)
      end

      assert_includes response, "101"
      socket.write("hi")
      socket.flush
      echoed = socket.readpartial(7)
      assert_equal "echo:hi", echoed
    ensure
      socket&.close
    end

    raise callback_error.pop unless callback_error.empty?
    stream_class, stream_is_io, read_arity = stream_details.pop
    assert_equal UNIXSocket, stream_class
    assert_equal true, stream_is_io
    assert_operator read_arity, :!=, 0
  end

  def test_async_websocket_rack_adapter_can_upgrade_and_echo_messages
    callback_error = Queue.new

    server(app_with_lint: lambda do |env|
      Async::WebSocket::Adapters::Rack.open(env) do |connection|
        begin
          message = connection.read
          connection.write(message)
          connection.flush
        rescue => e
          callback_error << e
        end
      end || [200, { "content-type" => "text/plain" }, ["ok"]]
    end) do |uri|
      socket = websocket_upgrade_socket(uri)
      connection = websocket_client_connection(socket)

      connection.write("hello")
      connection.flush

      message = connection.read
      assert_equal "hello", message.to_str
    ensure
      connection&.close rescue nil
      socket&.close
    end

    raise callback_error.pop unless callback_error.empty?
  end

  def test_enumerable_body
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "application/json" },
       %W[one\n two\n three\n]]
    end) do
      assert_equal "one\ntwo\nthree\n", get("/")
    end
  end

  def test_scheduler_non_blocking
    return unless RUBY_VERSION > "3.0"

    server(
      itsi_rb: lambda do
        fiber_scheduler "Itsi::Scheduler"
        run(lambda do |env|
          sleep 0.25
          [200, { "content-type" => "text/plain" }, "Response: #{env["PATH_INFO"][1..-1]}"]
        end)
      end
    ) do
      start_time = Time.now
      20.times.map do
        Thread.new do
          payload = SecureRandom.hex(16)
          response = get_resp("/#{payload}")
          assert_equal "Response: #{payload}", response.body
        end
      end.each(&:join)

      assert_in_delta 0.25, Time.now - start_time, 0.5
    end
  end

  def test_query_params
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "text/plain" }, [env["QUERY_STRING"]]]
    end) do
      assert_equal "foo=bar&baz=qux", get("/?foo=bar&baz=qux")
    end
  end

  def test_put_request
    server(app_with_lint: lambda do |env|
      body = env["rack.input"].read
      [200, { "content-type" => "text/plain" }, [body]]
    end) do |uri|
      req = Net::HTTP::Put.new(uri)
      req.body = "put data"
      response = Net::HTTP.start(uri.hostname, uri.port) { |http| http.request(req) }
      assert_equal "put data", response.body
    end
  end

  def test_custom_headers
    server(app_with_lint: lambda do |env|
      header = env["HTTP_X_CUSTOM"] || ""
      [200, { "content-type" => "text/plain" }, [header]]
    end) do |uri|
      req = Net::HTTP::Get.new(uri)
      req["X-Custom"] = "custom-value"
      response = Net::HTTP.start(uri.hostname, uri.port) { |http| http.request(req) }
      assert_equal "custom-value", response.body
    end
  end

  def test_error_response
    response = nil
    capture_subprocess_io do
      server(app_with_lint: lambda do |env|
        raise "Intentional error for testing"
      end) do
        response = get_resp("/")
      end
    end
    assert_equal "500", response.code
  end

  def test_redirect
    server(app_with_lint: lambda do |env|
      [302, { "location" => "http://example.com" }, []]
    end) do
      response = get_resp("/")
      assert_equal "302", response.code
      assert_equal "http://example.com", response["location"]
    end
  end

  def test_not_found
    server(app_with_lint: lambda do |env|
      if env["PATH_INFO"] == "/"
        [200, { "content-type" => "text/plain" }, ["Home"]]
      else
        [404, { "content-type" => "text/plain" }, ["Not Found"]]
      end
    end) do
      response = get_resp("/nonexistent")
      assert_equal "404", response.code
      assert_equal "Not Found", response.body
    end
  end

  def test_head_request
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "text/plain", "content-length" => "13" }, []]
    end) do
      response = head("/")
      assert_equal "200", response.code
      assert_empty response.body.to_s
      assert_equal "13", response["content-length"]
    end
  end

  def test_options_request
    server(app_with_lint: lambda do |env|
      [200, { "allow" => "GET,POST,OPTIONS", "content-type" => "text/plain" }, ["Options Response"]]
    end) do
      response = options("/")
      assert_equal "200", response.code
      assert_equal "GET,POST,OPTIONS", response["allow"]
      assert_equal "Options Response", response.body
    end
  end

  def test_cookie_handling
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "text/plain", "set-cookie" => "session=abc123; Path=/" }, ["Cookie Test"]]
    end) do
      response = get_resp("/")
      assert_equal "200", response.code
      assert_match(/session=abc123/, response["set-cookie"])
      assert_equal "Cookie Test", response.body
    end
  end

  def test_duplicate_cookie_request_headers_are_joined_for_rack
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "text/plain" }, [env["HTTP_COOKIE"].to_s]]
    end) do |uri|
      response = raw_http(
        "GET / HTTP/1.1\r\n" \
        "Host: #{uri.host}:#{uri.port}\r\n" \
        "Cookie: _session_id=abc\r\n" \
        "Cookie: _gat=1\r\n" \
        "Connection: close\r\n\r\n"
      )

      assert_includes response, "\r\n\r\n_session_id=abc; _gat=1"
    end
  end

  def test_multiple_headers
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "text/plain", "x-example" => "one, two, three" }, ["Multiple Headers"]]
    end) do
      response = get_resp("/")
      assert_equal "200", response.code
      assert_equal "one, two, three", response["x-example"]
      assert_equal "Multiple Headers", response.body
    end
  end

  def test_large_body
    large_text = "A" * 10_000
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "text/plain", "content-length" => large_text.bytesize.to_s }, [large_text]]
    end) do
      response = get_resp("/")
      assert_equal "200", response.code
      assert_equal large_text, response.body
    end
  end

  def test_custom_status_code
    server(app_with_lint: lambda do |env|
      [201, { "content-type" => "text/plain" }, ["Created"]]
    end) do
      response = get_resp("/")
      assert_equal "201", response.code
      assert_equal "Created", response.body
    end
  end

  def test_empty_body
    server(app_with_lint: lambda do |env|
      [204, {}, []]
    end) do
      response = get_resp("/")
      assert_equal "204", response.code
      assert_nil response.body
    end
  end

  def test_utf8_response
    utf8_text = "こんにちは世界"
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "text/plain; charset=utf-8" }, [utf8_text]]
    end) do
      response = get_resp("/")
      assert_equal "200", response.code
      assert_equal utf8_text, response.body.force_encoding("UTF-8")
    end
  end

  def test_custom_request_header
    server(app_with_lint: lambda do |env|
      header_value = env["HTTP_X_MY_HEADER"] || ""
      [200, { "content-type" => "text/plain" }, [header_value]]
    end) do |uri|
      req = Net::HTTP::Get.new(uri)
      req["X-My-Header"] = "test-header"
      response = Net::HTTP.start(uri.hostname, uri.port) { |http| http.request(req) }
      assert_equal "test-header", response.body
    end
  end

  def test_url_encoded_query_params
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "text/plain" }, [env["QUERY_STRING"]]]
    end) do
      assert_equal "param=%C3%A9", get("/?param=%C3%A9")
    end
  end

  def test_rackup_handler
    server(app_with_lint: lambda do |env|
      [200, { "content-type" => "text/plain" }, ["Hello, HTTPS!"]]
    end, protocol: "https") do |uri|
      response = Net::HTTP.start(uri.hostname, uri.port, use_ssl: true,
                                                         verify_mode: OpenSSL::SSL::VERIFY_NONE) do |http|
        http.request(Net::HTTP::Get.new("/"))
      end
      assert_equal "200", response.code
      assert_equal "Hello, HTTPS!", response.body
    end
  end

  # Used by `rails -s` and other tools using the rack-up interface.
  def test_rackup_handler
    host, port = free_bind.split(%r{:/?/?}).last(2)
    app = ->(_) { [200, { "content-type" => "text/plain" }, ["Hello, Rackup!"]] }

    Thread.new do
      Rack::Handler::Itsi.run(
        app,
        {
          host: host,
          Port: port
        }
      )
    end

    response = nil

    Timeout.timeout(2) do
      loop do
        begin
          response = Net::HTTP.get(URI("http://#{host}:#{port}"))
          break
        rescue Errno::ECONNREFUSED, Net::OpenTimeout
          sleep 0.05
        end
      end
    end

    assert_equal "Hello, Rackup!", response
    Process.kill(:SIGINT, Process.pid)
  end

  def test_script_name_inferred_from_mount
    server(itsi_rb: lambda do
      location "foo*" do
        run ->(env) { [200, { "content-type" => "text/plain" }, [env["SCRIPT_NAME"]]] }
      end
      run ->(env) { [200, { "content-type" => "text/plain" }, [env["SCRIPT_NAME"]]] }
    end) do
      assert_equal get("/foo/bar"), "/foo"
      assert_equal get("/baz"), ""
    end
  end

  def test_path_info_inferred_from_mount
    server(itsi_rb: lambda do
      location "foo*" do
        run ->(env) { [200, { "content-type" => "text/plain" }, [env["PATH_INFO"]]] }
      end
      run ->(env) { [200, { "content-type" => "text/plain" }, [env["PATH_INFO"]]] }
    end) do
      assert_equal get("/foo/bar"), "/bar"
      assert_equal get("/baz"), "/baz"
    end
  end

  def test_script_name_explicitly_set
    server(itsi_rb: lambda do
      location "foo*" do
        run ->(env) { [200, { "content-type" => "text/plain" }, [env["SCRIPT_NAME"]]] }, script_name: "/overridden"
      end
      run ->(env) { [200, { "content-type" => "text/plain" }, [env["SCRIPT_NAME"]]] }, script_name: ""
    end) do
      assert_equal get("/foo/bar"), "/overridden"
      assert_equal get("/baz"), ""
    end
  end

  def test_path_info_when_script_name_explicitly_set
    server(itsi_rb: lambda do
      location "foo*" do
        run ->(env) { [200, { "content-type" => "text/plain" }, [env["PATH_INFO"]]] }, script_name: ""
      end
      run ->(env) { [200, { "content-type" => "text/plain" }, [env["PATH_INFO"]]] }, script_name: ""
    end) do
      assert_equal get("/foo/bar"), "/foo/bar"
      assert_equal get("/baz"), "/baz"
    end
  end

  def test_multi_field_headers
    server(app_with_lint: lambda do |_|
      [200, { "content-type" => "text/plain", "x-example" => ["one, two, three", "four, five"] }, ["Multiple Field Headers"]]
    end) do
      response = get_resp("/")
      assert_equal "200", response.code
      assert_equal "one, two, three, four, five", response["x-example"]
      assert_equal "Multiple Field Headers", response.body
    end
  end

  # For backwards compatibility with Rack-2, which uses "\n" for multiple values in headers.
  # https://github.com/rack/rack/blob/df6c47357f6c6bec2d585f45f417285d813d9b3a/lib/rack/utils.rb#L271
  #
  # In Rack 3, the behavior has changed to using Arrays of Strings exclusively.
  # Note we don't use Rack lint here, because it'll complain about the invalid header value.
  def test_multiline_headers_legacy_cookie
    cookies = "one\r\ntwo\n three\n\tfour\nfive\n\n"
    server(app: ->(_) { [200, { "set-cookie" => cookies }, ["OK"]] }) do
      resp = get_resp("/")
      assert_equal "200", resp.code
      # folded lines should coalesce; empty lines disappear
      assert_equal %w[one two three four five], resp.get_fields("set-cookie")
    end
  end

  def test_control_chars_are_stripped
    evil = "good\nbad\x01bad\ngood"
    server(app: ->(_) { [200, { "x-evil" => evil }, ["body"]] }) do
      resp = get_resp("/")
      assert_equal %w[good good], resp.get_fields("x-evil")
    end
  end

  def test_rack3_array_is_untouched
    server(app: ->(_) { [200,
                        { "set-cookie" => ["a=b", "c=d"] }, ["OK"] ] }) do
      resp = get_resp("/")
      assert_equal %w[a=b c=d], resp.get_fields("set-cookie")
    end
  end

  private

  def websocket_upgrade_socket(uri)
    socket = TCPSocket.new(uri.host, uri.port)
    socket.write(
      "GET / HTTP/1.1\r\n" \
      "Host: #{uri.host}:#{uri.port}\r\n" \
      "Connection: Upgrade\r\n" \
      "Upgrade: websocket\r\n" \
      "Sec-WebSocket-Version: 13\r\n" \
      "Sec-WebSocket-Key: #{Base64.strict_encode64(SecureRandom.random_bytes(16))}\r\n" \
      "\r\n"
    )
    socket.flush

    response = +""
    until response.include?("\r\n\r\n")
      chunk = socket.read(1)
      raise EOFError, "Unexpected EOF during websocket handshake" unless chunk

      response << chunk
    end

    assert_includes response, "101"
    socket
  end

  def websocket_client_connection(socket)
    framer = Protocol::WebSocket::Framer.new(socket)
    Protocol::WebSocket::Connection.new(framer, mask: "\x01\x02\x03\x04")
  end
end
