require_relative "../helpers/test_helper"

class TestCORS < Minitest::Test

  def test_cors_supports_origin_and_methods
    server(
      itsi_rb: lambda do
        cors \
          allow_origins: ["https://itsi.fyi"],
          allow_methods: %w[GET POST],
          allow_headers: %w[Content-Type Authorization],
          allow_credentials: true,
          expose_headers: ["X-Special-Header"],
          max_age: 3600

        get("/foo") { |r| r.ok "ok" }
      end
    ) do
      res = options("/foo", {
        "Origin" => "https://itsi.fyi",
        "Access-Control-Request-Method" => "POST",
        "Access-Control-Request-Headers" => "Content-Type"
      })

      assert_includes res["Access-Control-Allow-Origin"], "https://itsi.fyi"
      assert_includes res["Access-Control-Allow-Methods"], "POST"
      assert_includes res["Access-Control-Allow-Headers"], "Content-Type"
      assert_equal "true", res["Access-Control-Allow-Credentials"]
      assert_equal "3600", res["Access-Control-Max-Age"]
    end
  end

  def test_cors_exposes_headers_and_allows_credentials
    server(
      itsi_rb: lambda do
        cors \
          allow_origins: ["https://itsi.fyi"],
          allow_methods: %w[GET],
          allow_headers: %w[Content-Type],
          allow_credentials: true,
          expose_headers: ["X-Special-Header"],
          max_age: 1000

        get("/foo") do |r|
          r.respond("ok", 200, {
            "X-Special-Header" => "42"
          })
        end
      end
    ) do
      res = get("/foo", {
        "Origin" => "https://itsi.fyi"
      })

      assert_equal "https://itsi.fyi", res["Access-Control-Allow-Origin"]
      assert_equal "true", res["Access-Control-Allow-Credentials"]
      assert_equal "X-Special-Header", res["Access-Control-Expose-Headers"]
    end
  end

  def test_cors_blocks_unauthorized_origin
    server(
      itsi_rb: lambda do
        cors \
          allow_origins: ["https://itsi.fyi"],
          allow_methods: %w[GET],
          allow_headers: %w[Content-Type],
          allow_credentials: false,
          expose_headers: [],
          max_age: 600

        get("/foo") { |r| r.ok "ok" }
      end
    ) do
      res = options("/foo", {
        "Origin" => "https://evil.com",
        "Access-Control-Request-Method" => "GET"
      })

      refute res.key?("Access-Control-Allow-Origin")
      refute res.key?("Access-Control-Allow-Headers")
      refute res.key?("Access-Control-Allow-Credentials")
    end
  end

  def test_cors_exposes_headers_and_allows_credentials
    server(
      itsi_rb: lambda do
        cors \
          allow_origins: ["https://itsi.fyi"],
          allow_methods: %w[GET],
          allow_headers: %w[Content-Type],
          allow_credentials: true,
          expose_headers: ["X-Special-Header"],
          max_age: 1000

        get("/foo") do |r|
          r.respond("ok", 200, {
            "X-Special-Header" => "42"
          })
        end
      end
    ) do
      res = get_resp("/foo", {
        "Origin" => "https://itsi.fyi"
      })
      assert_equal "https://itsi.fyi", res["Access-Control-Allow-Origin"]
      assert_equal "true", res["Access-Control-Allow-Credentials"]
      assert_equal "X-Special-Header", res["Access-Control-Expose-Headers"]
    end
  end

end
