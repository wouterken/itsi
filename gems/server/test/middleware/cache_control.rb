require_relative "../helpers/test_helper"

require_relative "../helpers/test_helper"

class TestCacheControl < Minitest::Test
  def test_cache_control_headers_are_set
    server(
      itsi_rb: lambda do
        cache_control \
          max_age: 3600,
          public: true,
          vary: ["Accept-Encoding"],
          additional_headers: { "X-Custom-Header" => "HIT" }
        get("/foo") { |r| r.ok "content" }
      end
    ) do
      res = get_resp("/foo")
      # Check that the Cache-Control header is present and contains required directives
      assert_includes res["Cache-Control"], "public"
      assert_includes res["Cache-Control"], "max-age=3600"

      # Check that the Expires header exists and is a valid HTTP date string
      assert res.key?("Expires"), "Expires header should be present"
      # A basic check: the Expires header should include a comma and a GMT designation
      assert_match /GMT/, res["Expires"]

      # Check that the Vary header is set correctly
      assert_equal "Accept-Encoding", res["Vary"]

      # Check that additional custom header is set
      assert_equal "HIT", res["X-Custom-Header"]
    end
  end

  def test_cache_control_not_set_for_error_statuses
    server(
      itsi_rb: lambda do
        cache_control max_age: 3600, public: true
        get("/foo") { |r| r.respond("error", 500) }
      end
    ) do
      res = get_resp("/foo")
      refute res.key?("Cache-Control"), "Cache-Control should not be set for 500 errors"
      refute res.key?("Expires"), "Expires should not be set for 500 errors"
    end
  end

  def test_cache_control_s_max_age_and_stale_directives
    server(
      itsi_rb: lambda do
        cache_control \
          max_age: 3600,
          s_max_age: 1800,
          stale_while_revalidate: 30,
          stale_if_error: 60,
          private: true
        get("/foo") { |r| r.ok "some content" }
      end
    ) do
      res = get_resp("/foo")
      cc = res["Cache-Control"]
      assert_includes cc, "private"
      assert_includes cc, "s-maxage=1800"
      assert_includes cc, "stale-while-revalidate=30"
      assert_includes cc, "stale-if-error=60"
    end
  end

  def test_cache_control_set_without_optional_fields
    server(
      itsi_rb: lambda do
        cache_control public: true
        get("/foo") { |r| r.ok "minimal" }
      end
    ) do
      res = get_resp("/foo")
      cc = res["Cache-Control"]
      assert_includes cc, "public"
    end
  end
end
