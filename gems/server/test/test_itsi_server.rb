require_relative "helpers/test_helper"

class TestItsiServer < Minitest::Test
  def test_that_it_has_a_version_number
    refute_nil ::Itsi::Server::VERSION
  end

  def test_hello_world
    server(app: lambda do |env|
      [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
    end) do
      assert_equal "Hello, World!", get("/")
    end
  end

  def test_post
    server(app: lambda do |env|
      assert_equal env["REQUEST_METHOD"], "POST"
      assert_equal "data", env["rack.input"].read
      [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
    end) do
      assert_equal "Hello, World!", post("/", "data").body
    end
  end

  def test_full_hijack
    server(app: lambda do |env|
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
    end) do
      assert_equal "Hello, World!", get("/")
    end
  end

  def test_streaming_body
    server(app: lambda do |env|
      [200, { "Content-Type" => "text/plain" }, lambda { |stream|
        stream.write("Hello")
        stream.write(", World!")
        stream.close
      }]
    end) do
      assert_equal "Hello, World!", get("/")
    end
  end

  def test_partial_hijack
    server(app: lambda do |env|
      [200, { "Content-Type" => "text/plain", "rack.hijack" => lambda { |stream|
        stream.write("Hello")
        stream.write(", World!")
        stream.close
      } }, []]
    end) do
      assert_equal "Hello, World!", get("/")
    end
  end

  def test_enumerable_body
    server(app: lambda do |env|
      [200, { "Content-Type" => "application/json" },
       %W[one\n two\n three\n]]
    end) do
      assert_equal "one\ntwo\nthree\n", get("/")
    end
  end

  def test_scheduler_non_blocking
    server(
      itsi_rb: lambda do
        fiber_scheduler "Itsi::Scheduler"
        run(lambda do |env|
          sleep 0.25
          [200, { "Content-Type" => "text/plain" }, "Response: #{env["PATH_INFO"][1..-1]}"]
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
    server(app: lambda do |env|
      [200, { "Content-Type" => "text/plain" }, [env["QUERY_STRING"]]]
    end) do
      assert_equal "foo=bar&baz=qux", get("/?foo=bar&baz=qux")
    end
  end

  def test_put_request
    server(app: lambda do |env|
      body = env["rack.input"].read
      [200, { "Content-Type" => "text/plain" }, [body]]
    end) do |uri|
      req = Net::HTTP::Put.new(uri)
      req.body = "put data"
      response = Net::HTTP.start(uri.hostname, uri.port) { |http| http.request(req) }
      assert_equal "put data", response.body
    end
  end

  def test_custom_headers
    server(app: lambda do |env|
      header = env["HTTP_X_CUSTOM"] || ""
      [200, { "Content-Type" => "text/plain" }, [header]]
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
      server(app: lambda do |env|
        raise "Intentional error for testing"
      end) do
        response = get_resp("/")
      end
    end
    assert_equal "500", response.code
  end

  def test_redirect
    server(app: lambda do |env|
      [302, { "Location" => "http://example.com" }, []]
    end) do
      response = get_resp("/")
      assert_equal "302", response.code
      assert_equal "http://example.com", response["location"]
    end
  end

  def test_not_found
    server(app: lambda do |env|
      if env["PATH_INFO"] == "/"
        [200, { "Content-Type" => "text/plain" }, ["Home"]]
      else
        [404, { "Content-Type" => "text/plain" }, ["Not Found"]]
      end
    end) do
      response = get_resp("/nonexistent")
      assert_equal "404", response.code
      assert_equal "Not Found", response.body
    end
  end

  def test_head_request
    server(app: lambda do |env|
      [200, { "Content-Type" => "text/plain", "Content-Length" => "13" }, ["Hello, World!"]]
    end) do
      response = head("/")
      assert_equal "200", response.code
      assert_empty response.body.to_s
      assert_equal "13", response["content-length"]
    end
  end

  def test_options_request
    server(app: lambda do |env|
      [200, { "Allow" => "GET,POST,OPTIONS", "Content-Type" => "text/plain" }, ["Options Response"]]
    end) do
      response = options("/")
      assert_equal "200", response.code
      assert_equal "GET,POST,OPTIONS", response["allow"]
      assert_equal "Options Response", response.body
    end
  end

  def test_cookie_handling
    server(app: lambda do |env|
      [200, { "Content-Type" => "text/plain", "Set-Cookie" => "session=abc123; Path=/" }, ["Cookie Test"]]
    end) do
      response = get_resp('/')
      assert_equal "200", response.code
      assert_match(/session=abc123/, response["set-cookie"])
      assert_equal "Cookie Test", response.body
    end
  end

  def test_multiple_headers
    server(app: lambda do |env|
      [200, { "Content-Type" => "text/plain", "X-Example" => "one, two, three" }, ["Multiple Headers"]]
    end) do
      response = get_resp("/")
      assert_equal "200", response.code
      assert_equal "one, two, three", response["x-example"]
      assert_equal "Multiple Headers", response.body
    end
  end

  def test_large_body
    large_text = "A" * 10_000
    server(app: lambda do |env|
      [200, { "Content-Type" => "text/plain", "Content-Length" => large_text.bytesize.to_s }, [large_text]]
    end) do
      response = get_resp("/")
      assert_equal "200", response.code
      assert_equal large_text, response.body
    end
  end

  def test_custom_status_code
    server(app: lambda do |env|
      [201, { "Content-Type" => "text/plain" }, ["Created"]]
    end) do
      response = get_resp("/")
      assert_equal "201", response.code
      assert_equal "Created", response.body
    end
  end

  def test_empty_body
    server(app: lambda do |env|
      [204, { "Content-Type" => "text/plain" }, []]
    end) do
      response = get_resp("/")
      assert_equal "204", response.code
      assert_nil response.body
    end
  end

  def test_utf8_response
    utf8_text = "こんにちは世界"
    server(app: lambda do |env|
      [200, { "Content-Type" => "text/plain; charset=utf-8" }, [utf8_text]]
    end) do
      response = get_resp("/")
      assert_equal "200", response.code
      assert_equal utf8_text, response.body.force_encoding("UTF-8")
    end
  end

  def test_custom_request_header
    server(app: lambda do |env|
      header_value = env["HTTP_X_MY_HEADER"] || ""
      [200, { "Content-Type" => "text/plain" }, [header_value]]
    end) do |uri|

      req = Net::HTTP::Get.new(uri)
      req["X-My-Header"] = "test-header"
      response = Net::HTTP.start(uri.hostname, uri.port) { |http| http.request(req) }
      assert_equal "test-header", response.body
    end
  end

  def test_url_encoded_query_params
    server(app: lambda do |env|
      [200, { "Content-Type" => "text/plain" }, [env["QUERY_STRING"]]]
    end) do
      assert_equal "param=%C3%A9", get("/?param=%C3%A9")
    end
  end

  def test_https
    server(app: lambda do |env|
      [200, { "Content-Type" => "text/plain" }, ["Hello, HTTPS!"]]
    end, protocol: "https") do |uri|
      response = Net::HTTP.start(uri.hostname, uri.port, use_ssl: true,
                                                         verify_mode: OpenSSL::SSL::VERIFY_NONE) do |http|
        http.request(Net::HTTP::Get.new("/"))
      end
      assert_equal "200", response.code
      assert_equal "Hello, HTTPS!", response.body
    end
  end
end
