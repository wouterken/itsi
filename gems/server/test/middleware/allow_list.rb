# test_allow_list.rb
require_relative "../helpers/test_helper"

class TestAllowList < Minitest::Test
  # 1. Single‐pattern: only localhost is allowed
  def test_single_pattern_allows_and_denies
    server(
      itsi_rb: lambda do
        allow_list allowed_patterns: ["^127\\.0\\.0\\.1$"]
        get("/foo") { |r| r.ok "allowed" }
      end
    ) do
      # Our test client always comes from 127.0.0.1
      res1 = get_resp("/foo")
      assert_equal "200", res1.code
      assert_equal "allowed", res1.body

      # If we change the pattern so it no longer matches, 127.0.0.1 is now forbidden
      server(
        itsi_rb: lambda do
          allow_list allowed_patterns: ["^10\\.0\\."]
          get("/foo") { |r| r.ok "never" }
        end
      ) do
        res2 = get_resp("/foo")
        assert_equal "403", res2.code
      end
    end
  end

  # 2. Multiple‐pattern: localhost or 192.168.x.x
  def test_multiple_patterns
    server(
      itsi_rb: lambda do
        allow_list allowed_patterns: ["^127\\.0\\.0\\.1$", "^192\\.168\\."]
        get("/ping") { |r| r.ok "pong" }
      end
    ) do
      # localhost matches first pattern
      res1 = get_resp("/ping")
      assert_equal "200", res1.code
      assert_equal "pong", res1.body

      # If we restrict to only 192.168.*, localhost becomes forbidden
      server(
        itsi_rb: lambda do
          allow_list allowed_patterns: ["^192\\.168\\."]
          get("/ping") { |r| r.ok "never" }
        end
      ) do
        res2 = get_resp("/ping")
        assert_equal "403", res2.code
      end
    end
  end

  # 3. Custom error_response
  def test_custom_error_response
    server(
      itsi_rb: lambda do
        allow_list \
          allowed_patterns: ["^192\\.168\\."],  # localhost no longer matches
          error_response: {
            code: 403,
            plaintext: { inline: "No access" },
            default: "plaintext"
          }
        get("/x") { |r| r.ok "never" }
      end
    ) do
      res = get_resp("/x")
      assert_equal "403", res.code
      assert_equal "No access", res.body
    end
  end
end
