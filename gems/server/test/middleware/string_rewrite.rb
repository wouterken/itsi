require_relative "../helpers/test_helper"

class TestStringRewrite < Minitest::Test

  def test_interpolates_method
    server(
      itsi_rb: lambda do
        redirect to: "https://example.com/{method}/redirect", type: "temporary"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "307", res.code, "Expected status 307 for temporary redirect"
      assert_equal "https://example.com/GET/redirect", res["Location"]
    end
  end

  def test_interpolates_path
    server(
      itsi_rb: lambda do
        redirect to: "https://example.com{path}", type: "found"
        get("/bar") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/bar")
      assert_equal "302", res.code, "Expected status 302 for found redirect"
      assert_equal "https://example.com/bar", res["Location"]
    end
  end

  def test_interpolates_addr
    server(
      itsi_rb: lambda do
        redirect to: "https://{addr}/redirect", type: "moved_permanently"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "301", res.code, "Expected status 301 for moved permanently redirect"
      # In tests, the context's address is typically set to "127.0.0.1"
      assert_equal "https://127.0.0.1/redirect", res["Location"]
    end
  end

  def test_interpolates_host
    server(
      itsi_rb: lambda do
        redirect to: "https://{host}/", type: "permanent"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "308", res.code, "Expected status 308 for permanent redirect"
      # When no host is explicitly specified in the URL, it defaults to "localhost"
      assert_equal "https://localhost/", res["Location"]
    end
  end

  def test_interpolates_path_and_query
    server(
      itsi_rb: lambda do
        redirect to: "https://example.com{path_and_query}", type: "temporary"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo?bar=baz")
      assert_equal "307", res.code, "Expected status 307 for temporary redirect"
      assert_equal "https://example.com/foo?bar=baz", res["Location"]
    end
  end

  def test_interpolates_query
    server(
      itsi_rb: lambda do
        redirect to: "https://example.com{query}", type: "found"
        get("/foo?x=y") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo?x=y")
      assert_equal "302", res.code, "Expected status 302 for found redirect"
      assert_equal "https://example.com?x=y", res["Location"]
    end
  end

  def test_interpolates_port
    server(
      itsi_rb: lambda do
        redirect to: "https://example.com:{port}/", type: "moved_permanently"
        # We assume that if no port is provided in the request URI, it defaults to "80"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "301", res.code, "Expected status 301 for moved permanently redirect"
      assert_equal "https://example.com:80/", res["Location"]
    end
  end

  def test_unknown_placeholder_remains_unmodified
    server(
      itsi_rb: lambda do
        redirect to: "https://example.com/{unknown}", type: "permanent"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo")
      # The unknown placeholder is not substituted and remains as '{unknown}'.
      assert_equal "308", res.code, "Expected status 308 for permanent redirect"
      assert_equal "https://example.com/{unknown}", res["Location"]
    end
  end

  def test_strip_prefix
    server(
      itsi_rb: lambda do
        redirect to: "https://example.com{path|strip_prefix:/rails}", type: "temporary"
        get("/rails/foo") { |r| r.ok }
      end
    ) do
      res = get_resp("/rails/foo")
      assert_equal "307", res.code
      assert_equal "https://example.com/foo", res["Location"]
    end
  end

  def test_strip_suffix
    server(
      itsi_rb: lambda do
        redirect to: "{path_and_query|strip_suffix:.json}", type: "found"
        get("/data.json") { |r| r.ok }
      end
    ) do
      res = get_resp("/data.json")
      assert_equal "302", res.code
      # .json is removed from end of "/data.json"
      assert_equal "/data", res["Location"]
    end
  end

  def test_replace
    server(
      itsi_rb: lambda do
        redirect to: "https://{host|replace:localhost,api.example.com}", type: "permanent"
        get("/foo") { |r| r.ok }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "308", res.code
      assert_equal "https://api.example.com", res["Location"]
    end
  end

  def test_chain_modifiers
    server(
      itsi_rb: lambda do
        redirect to: "https://example.com{path|strip_prefix:/v1|replace:api,service}", type: "temporary"
        get("/v1/api/users") { |r| r.ok }
      end
    ) do
      res = get_resp("/v1/api/users")
      assert_equal "307", res.code
      # after strip_prefix: "/api/users", then replace "api"→"service"
      assert_equal "https://example.com/service/users", res["Location"]
    end
  end
end
