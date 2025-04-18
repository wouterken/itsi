# test_deny_list.rb
require_relative "../helpers/test_helper"

class TestDenyList < Minitest::Test
  # 1. Single‐pattern: localhost is denied
  def test_single_pattern_denies_and_allows
    server(
      itsi_rb: lambda do
        deny_list denied_patterns: ["^127\\.0\\.0\\.1$"]
        get("/foo") { |r| r.ok "ok" }
      end
    ) do
      # Client from 127.0.0.1 is denied
      res1 = get_resp("/foo")
      assert_equal "403", res1.code

      # If we change the pattern to something else, localhost is allowed
      server(
        itsi_rb: lambda do
          deny_list denied_patterns: ["^10\\."]
          get("/foo") { |r| r.ok "allowed" }
        end
      ) do
        res2 = get_resp("/foo")
        assert_equal "200", res2.code
        assert_equal "allowed", res2.body
      end
    end
  end

  # 2. Multiple‐pattern: deny localhost or 172.16.x.x
  def test_multiple_patterns
    server(
      itsi_rb: lambda do
        deny_list denied_patterns: ["^127\\.0\\.0\\.1$", "^172\\.16\\."]
        get("/h") { |r| r.ok "h" }
      end
    ) do
      # localhost matches first pattern → denied
      res1 = get_resp("/h")
      assert_equal "403", res1.code

      # If we restrict only to 172.16.*, localhost becomes allowed
      server(
        itsi_rb: lambda do
          deny_list denied_patterns: ["^172\\.16\\."]
          get("/h") { |r| r.ok "ok" }
        end
      ) do
        res2 = get_resp("/h")
        assert_equal "200", res2.code
        assert_equal "ok",   res2.body
      end
    end
  end

  # 3. Custom error_response
  def test_custom_error_response
    server(
      itsi_rb: lambda do
        deny_list \
          denied_patterns: ["^127\\.0\\.0\\.1$"],
          error_response: {
            code: 403,
            plaintext: { inline: "Blocked by IP" },
            default: "plaintext"
          }
        get("/z") { |r| r.ok "never" }
      end
    ) do
      res = get_resp("/z")
      assert_equal "403", res.code
      assert_equal "Blocked by IP", res.body
    end
  end
end
