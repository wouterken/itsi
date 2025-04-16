require_relative "../helpers/test_helper"

class TestRedirect < Minitest::Test
  def test_permanent_redirect
    server(
      itsi_rb: lambda do
        redirect to: "https://example.com/new", type: "moved_permanently"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "301", res.code, "Expected status 301 for permanent redirect"
      assert_equal "https://example.com/new", res["Location"]
    end
  end

  def test_temporary_redirect
    server(
      itsi_rb: lambda do
        redirect to: "https://example.com/new", type: "temporary"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "307", res.code, "Expected status 307 for temporary redirect"
      assert_equal "https://example.com/new", res["Location"]
    end
  end

  def test_found_redirect
    server(
      itsi_rb: lambda do
        redirect to: "https://example.com/new", type: "found"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "302", res.code, "Expected status 302 for found redirect"
      assert_equal "https://example.com/new", res["Location"]
    end
  end

  def test_moved_permanently_redirect
    server(
      itsi_rb: lambda do
        redirect to: "https://example.com/new", type: "permanent"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "308", res.code, "Expected status 308 for moved permanently redirect"
      assert_equal "https://example.com/new", res["Location"]
    end
  end

  def test_relative_redirect
    server(
      itsi_rb: lambda do
        # Use a relative URL as the target
        redirect to: "/new/path", type: "permanent"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "308", res.code, "Expected status 308 for permanent redirect"
      # For a relative URL, the Location header should match exactly
      assert_equal "/new/path", res["Location"]
    end
  end

  def test_relative_redirect_with_placeholder
    server(
      itsi_rb: lambda do
        # Use a template that interpolates the request path into a relative URL.
        # For a request to "/foo", the resulting Location should be "/new/foo".
        redirect to: "/new{path}", type: "temporary"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "307", res.code, "Expected status 307 for temporary redirect"
      assert_equal "/new/foo", res["Location"]
    end
  end
end
