require_relative "../helpers/test_helper"

class TestRequestHeaders < Minitest::Test
  def test_add_and_remove_static
    server(
      itsi_rb: lambda do
        request_headers \
          additions: { "X-Test" => ["hello"] },
          removals:  ["X-Remove-Me"]
        get("/foo") do |r|
          r.ok r.header("X-Test").first + "|" + (r.header("X-Remove-Me").empty? ? "gone" : "here")
        end
      end
    ) do
      res = get_resp("/foo", { "X-Remove-Me" => "bye" })
      assert_equal "200", res.code
      assert_equal "hello|gone", res.body
    end
  end

  def test_dynamic_string_rewrite
    server(
      itsi_rb: lambda do
        request_headers \
          additions: { "X-Path" => ["{path_and_query}"] },
          removals:  []
        get("/bar") { |r| r.ok r.header("X-Path").first }
      end
    ) do
      res = get_resp("/bar?x=1")
      assert_equal "200", res.code
      assert_equal "/bar?x=1", res.body
    end
  end

  def test_override_existing_header
    server(
      itsi_rb: lambda do
        request_headers \
          additions: { "User-Agent" => ["ItsiTester"] },
          removals:  ["User-Agent"]
        get("/u") { |r| r.ok r.header("User-Agent").first }
      end
    ) do
      res = get_resp("/u", { "User-Agent" => "orig" })
      assert_equal "200", res.code
      assert_equal "ItsiTester", res.body
    end
  end
end
