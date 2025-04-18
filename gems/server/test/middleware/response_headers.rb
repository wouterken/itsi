require_relative "../helpers/test_helper"

class TestResponseHeaders < Minitest::Test
  def test_add_and_remove_static
    server(
      itsi_rb: lambda do
        response_headers \
          additions: { "X-Test" => ["world"] }
          # removals:  ["X-Custom"]
        get("/foo") { |r| r.ok "hi", {"X-Custom" => "value"} }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "200", res.code
      assert_equal "world", res["X-Test"]
      refute res.key?("X-Custom")
    end
  end

  def test_dynamic_response_rewrite
    server(
      itsi_rb: lambda do
        response_headers \
          additions: { "X-Addr" => ["{addr}"] },
          removals:  []
        get("/addr") { |r| r.ok "12345" }
      end
    ) do
      res = get_resp("/addr")
      assert_equal "200", res.code
      assert_equal "127.0.0.1", res["X-Addr"]
    end
  end

  def test_override_multiple_values
    server(
      itsi_rb: lambda do
        response_headers \
          additions: { "Set-Cookie" => ["a=1", "b=2"] },
          removals:  ["Set-Cookie"]
        get("/cookie") { |r| r.ok "ok" }
      end
    ) do
      res = get_resp("/cookie")
      cookies = res.get_fields("Set-Cookie")
      assert_includes cookies, "a=1"
      assert_includes cookies, "b=2"
    end
  end
end
