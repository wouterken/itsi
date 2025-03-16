require "socket"
require "net/http"
require "minitest/autorun"

class TestItsiServer < Minitest::Test
  def test_that_it_has_a_version_number
    refute_nil ::Itsi::Server::VERSION
  end

  def test_hello_world
    run_app(lambda do |env|
      [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
    end) do |uri|
      assert_equal "Hello, World!", Net::HTTP.get(uri)
    end
  end

  def test_post
    run_app(lambda do |env|
      assert_equal env["REQUEST_METHOD"], "POST"
      assert_equal "data", env["rack.input"].read
      [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
    end) do |uri|
      assert_equal "Hello, World!", Net::HTTP.post(uri, "data").body
    end
  end

  def test_full_hijack
    run_app(lambda do |env|
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
    end) do |uri|
      assert_equal "Hello, World!", Net::HTTP.get(uri)
    end
  end

  def test_streaming_body
    run_app(lambda do |env|
      [200, { "Content-Type" => "text/plain" }, lambda { |stream|
        stream.write("Hello")
        stream.write(", World!")
        stream.close
      }]
    end) do |uri|
      assert_equal "Hello, World!", Net::HTTP.get(uri)
    end
  end

  def test_partial_hijack
    run_app(lambda do |env|
      [200, { "Content-Type" => "text/plain", "rack.hijack" => lambda { |stream|
        stream.write("Hello")
        stream.write(", World!")
        stream.close
      } }, []]
    end) do |uri|
      assert_equal "Hello, World!", Net::HTTP.get(uri)
    end
  end

  def test_enumerable_body
    run_app(lambda do |env|
      [200, { "Content-Type" => "application/json" },
       %W[one\n two\n three\n]]
    end) do |uri|
      assert_equal "one\ntwo\nthree\n", Net::HTTP.get(uri)
    end
  end

  def test_scheduler_non_blocking
    run_app(
      lambda do |env|
        sleep 0.25
        [200, { "Content-Type" => "text/plain" }, "Hello, World!"]
      end,
      scheduler_class: "Itsi::Scheduler"
    ) do |uri|
      start_time = Time.now
      20.times.map do
        Thread.new do
          assert_equal "Hello, World!", Net::HTTP.get(uri)
        end
      end.each(&:join)
      assert_in_delta 0.25, Time.now - start_time, 0.5
    end
  end

  def test_query_params
    run_app(lambda do |env|
      [200, { "Content-Type" => "text/plain" }, [env["QUERY_STRING"]]]
    end) do |uri|
      uri.query = "foo=bar&baz=qux"
      assert_equal "foo=bar&baz=qux", Net::HTTP.get(uri)
    end
  end

  def test_put_request
    run_app(lambda do |env|
      body = env["rack.input"].read
      [200, { "Content-Type" => "text/plain" }, [body]]
    end) do |uri|
      uri_obj = URI(uri)
      req = Net::HTTP::Put.new(uri_obj)
      req.body = "put data"
      response = Net::HTTP.start(uri_obj.hostname, uri_obj.port) { |http| http.request(req) }
      assert_equal "put data", response.body
    end
  end

  def test_custom_headers
    run_app(lambda do |env|
      header = env["HTTP_X_CUSTOM"] || ""
      [200, { "Content-Type" => "text/plain" }, [header]]
    end) do |uri|
      uri_obj = URI(uri)
      req = Net::HTTP::Get.new(uri_obj)
      req["X-Custom"] = "custom-value"
      response = Net::HTTP.start(uri_obj.hostname, uri_obj.port) { |http| http.request(req) }
      assert_equal "custom-value", response.body
    end
  end

  def test_error_response
    response = nil
    capture_subprocess_io do
      run_app(lambda do |env|
        raise "Intentional error for testing"
      end) do |uri|
        response = Net::HTTP.get_response(uri)
      end
    end
    assert_equal "500", response.code
  end

  def test_redirect
    run_app(lambda do |env|
      [302, { "Location" => "http://example.com" }, []]
    end) do |uri|
      response = Net::HTTP.get_response(uri)
      assert_equal "302", response.code
      assert_equal "http://example.com", response["location"]
    end
  end

  def test_not_found
    run_app(lambda do |env|
      if env["PATH_INFO"] == "/"
        [200, { "Content-Type" => "text/plain" }, ["Home"]]
      else
        [404, { "Content-Type" => "text/plain" }, ["Not Found"]]
      end
    end) do |uri|
      uri.path = "/nonexistent"
      response = Net::HTTP.get_response(uri)
      assert_equal "404", response.code
      assert_equal "Not Found", response.body
    end
  end

  def test_head_request
    run_app(lambda do |env|
      [200, { "Content-Type" => "text/plain", "Content-Length" => "13" }, ["Hello, World!"]]
    end) do |uri|
      uri_obj = URI(uri)
      response = Net::HTTP.start(uri_obj.hostname, uri_obj.port) do |http|
        http.head("/")
      end
      assert_equal "200", response.code
      assert_empty response.body.to_s
      assert_equal "13", response["content-length"]
    end
  end

  def test_options_request
    run_app(lambda do |env|
      [200, { "Allow" => "GET,POST,OPTIONS", "Content-Type" => "text/plain" }, ["Options Response"]]
    end) do |uri|
      uri_obj = URI(uri)
      req = Net::HTTP::Options.new(uri_obj)
      response = Net::HTTP.start(uri_obj.hostname, uri_obj.port) { |http| http.request(req) }
      assert_equal "200", response.code
      assert_equal "GET,POST,OPTIONS", response["allow"]
      assert_equal "Options Response", response.body
    end
  end

  def test_cookie_handling
    run_app(lambda do |env|
      [200, { "Content-Type" => "text/plain", "Set-Cookie" => "session=abc123; Path=/" }, ["Cookie Test"]]
    end) do |uri|
      response = Net::HTTP.get_response(uri)
      assert_equal "200", response.code
      assert_match(/session=abc123/, response["set-cookie"])
      assert_equal "Cookie Test", response.body
    end
  end

  def test_multiple_headers
    run_app(lambda do |env|
      [200, { "Content-Type" => "text/plain", "X-Example" => "one, two, three" }, ["Multiple Headers"]]
    end) do |uri|
      response = Net::HTTP.get_response(uri)
      assert_equal "200", response.code
      assert_equal "one, two, three", response["x-example"]
      assert_equal "Multiple Headers", response.body
    end
  end

  def test_large_body
    large_text = "A" * 10_000
    run_app(lambda do |env|
      [200, { "Content-Type" => "text/plain", "Content-Length" => large_text.bytesize.to_s }, [large_text]]
    end) do |uri|
      response = Net::HTTP.get_response(uri)
      assert_equal "200", response.code
      assert_equal large_text, response.body
    end
  end

  def test_custom_status_code
    run_app(lambda do |env|
      [201, { "Content-Type" => "text/plain" }, ["Created"]]
    end) do |uri|
      response = Net::HTTP.get_response(uri)
      assert_equal "201", response.code
      assert_equal "Created", response.body
    end
  end

  def test_empty_body
    run_app(lambda do |env|
      [204, { "Content-Type" => "text/plain" }, []]
    end) do |uri|
      response = Net::HTTP.get_response(uri)
      assert_equal "204", response.code
      assert_nil response.body
    end
  end

  def test_utf8_response
    utf8_text = "こんにちは世界"
    run_app(lambda do |env|
      [200, { "Content-Type" => "text/plain; charset=utf-8" }, [utf8_text]]
    end) do |uri|
      response = Net::HTTP.get_response(uri)
      assert_equal "200", response.code
      assert_equal utf8_text, response.body.force_encoding("UTF-8")
    end
  end

  def test_custom_request_header
    run_app(lambda do |env|
      header_value = env["HTTP_X_MY_HEADER"] || ""
      [200, { "Content-Type" => "text/plain" }, [header_value]]
    end) do |uri|
      uri_obj = URI(uri)
      req = Net::HTTP::Get.new(uri_obj)
      req["X-My-Header"] = "test-header"
      response = Net::HTTP.start(uri_obj.hostname, uri_obj.port) { |http| http.request(req) }
      assert_equal "test-header", response.body
    end
  end

  def test_url_encoded_query_params
    run_app(lambda do |env|
      [200, { "Content-Type" => "text/plain" }, [env["QUERY_STRING"]]]
    end) do |uri|
      uri.query = "param=%C3%A9" # %C3%A9 represents 'é'
      assert_equal "param=%C3%A9", Net::HTTP.get(uri)
    end
  end
end
