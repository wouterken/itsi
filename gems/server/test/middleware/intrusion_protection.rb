require_relative "../helpers/test_helper"
require "redis"

class TestIntrusionProtection < Minitest::Test

  # 1. Banned URL pattern causes immediate ban + 403
  def test_banned_url_pattern
    server(
      itsi_rb: lambda do
        location "/secret" do
          intrusion_protection \
            banned_url_patterns: ["/secret"],
            banned_time_seconds: 0.1,
            store_config: { redis: { connection_url: "redis://localhost:6379/14" } }
          get { |r| r.ok "never" }
        end
        location "/public" do
          get { |r| r.ok "public" }
        end
      end
    ) do

      # First request: matched → banned + forbidden
      res1 = get_resp("/secret")
      assert_equal "403", res1.code

      # Immediately banned: second request also forbidden
      res2 = get_resp("/secret")
      assert_equal "403", res2.code

      # After ban TTL expires
      sleep 0.11

      res3 = get_resp("/public")
      assert_equal "200", res3.code
      res4 = get_resp("/secret")
      assert_equal "403", res4.code
    end
  end

  # 2. Banned header pattern causes ban + 403
  def test_banned_header_pattern
    server(
      itsi_rb: lambda do
        intrusion_protection \
          banned_header_patterns: { "User-Agent" => ["BadBot"] },
          banned_time_seconds: 0.1,
          store_config: "in_memory"
        get("/hi") { |r| r.ok "hi" }
      end
    ) do
      # First: header matches → banned
      res1 = get_resp("/hi", { "User-Agent" => "MyBadBot/1.0" })
      assert_equal "403", res1.code

      # Still banned until TTL
      res2 = get_resp("/hi", { "User-Agent" => "MyBadBot/1.0" })
      assert_equal "403", res2.code

      sleep 0.11
      # After TTL, banned set clears; header still matches → ban again
      res3 = get_resp("/hi", { "User-Agent" => "MyBadBot/1.0" })
      assert_equal "403", res3.code
    end
  end

  # 3. Clean traffic passes through
  def test_clean_traffic
    server(
      itsi_rb: lambda do
        intrusion_protection \
          banned_url_patterns: [".*\\.php$"],
          banned_header_patterns: { "X-Test" => ["evil"] },
          banned_time_seconds: 0.1,
          store_config: { redis: { connection_url: "redis://localhost:6379/13" } }
        get("/hello") { |r| r.ok "world" }
      end
    ) do
      # Non‑matching URL & header → allowed
      res = get_resp("/hello", { "User-Agent" => "Mozilla/5.0" })
      assert_equal "200", res.code
      assert_equal "world", res.body
    end
  end

  # 4. Custom error_response
  def test_custom_error_response
    server(
      itsi_rb: lambda do
        intrusion_protection \
          banned_url_patterns: ["/bad"],
          banned_header_patterns: {},
          banned_time_seconds: 5,
          store_config: "in_memory",
          error_response: {
            code: 401,
            plaintext: { inline: "Halt!" },
            default: "plaintext"
          }
        get("/bad") { |r| r.ok "never" }
      end
    ) do
      res = get_resp("/bad")
      assert_equal "401", res.code
      assert_equal "Halt!", res.body
    end
  end

  # 5. Intrusion protection middleware stacks (nested: parent + child)
  def test_nested_intrusion_protection_stacking
    server(
      itsi_rb: lambda do
        location "/protected" do
          intrusion_protection \
            banned_url_patterns: ["/nested"],
            banned_time_seconds: 0.1,
            store_config: "in_memory"

          location "/nested" do
            intrusion_protection \
              banned_header_patterns: { "X-Evil" => ["1"] },
              banned_time_seconds: 0.1,
              store_config: "in_memory"
            get { |r| r.ok "should not see this" }
          end

          get { |r| r.ok "Should also not see this" }
        end
        location "/public" do
          get { |r| r.ok "safe" }
        end
      end
    ) do
      # 1. Triggers child (header) rule → ban
      res1 = get_resp("protected/nested", { "X-Evil" => "1" })
      assert_equal "403", res1.code

      sleep 0.11

      # 3. Triggers parent (path) rule → ban
      res3 = get_resp("protected/nested")
      assert_equal "403", res3.code

      sleep 0.11

      # 4. Confirm public route works
      res4 = get_resp("/public")
      assert_equal "200", res4.code
    end
  end

  # 6. Sibling intrusion protection rules stack independently
  def test_sibling_intrusion_protection_stacking
    server(
      itsi_rb: lambda do
        location "/one" do
          intrusion_protection \
            banned_url_patterns: ["/one"],
            banned_time_seconds: 0.1,
            store_config: "in_memory"
          get { |r| r.ok "never" }
        end

        location "/two" do
          intrusion_protection \
            banned_header_patterns: { "X-Bot" => ["true"] },
            banned_time_seconds: 0.1,
            store_config: "in_memory"
          get { |r| r.ok "never" }
        end

        location "/ok" do
          get { |r| r.ok "ok" }
        end
      end
    ) do
      # Route `/one` banned by path
      res1 = get_resp("/one")
      assert_equal "403", res1.code

      # Route `/two` banned by header
      res2 = get_resp("/two", { "X-Bot" => "true" })
      assert_equal "403", res2.code

      # Route `/ok` untouched
      res3 = get_resp("/ok")
      assert_equal "200", res3.code

      # Wait for TTL to expire, confirm unban
      sleep 0.11

      # `/one` still banned due to path match → re-ban
      res4 = get_resp("/one")
      assert_equal "403", res4.code

      # `/two` still banned by header → re-ban
      res5 = get_resp("/two", { "X-Bot" => "true" })
      assert_equal "403", res5.code
    end
  end
end
