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

  # 4. Trusted proxies: extract client IP from header if proxy is trusted
   def test_trusted_proxy_allows_based_on_header
     server(
       itsi_rb: lambda do
         allow_list \
           allowed_patterns: ["^203\\.0\\.113\\.7$"],  # only allow this client IP
           trusted_proxies: {
             "127.0.0.1" => { header: { name: "X-Forwarded-For" } }
           }
         get("/trusted") { |r| r.ok "trusted" }
       end
     ) do
       res = get_resp("/trusted", { "X-Forwarded-For" => "203.0.113.7" })
       assert_equal "200", res.code
       assert_equal "trusted", res.body
     end
   end

   def test_trusted_proxy_denies_if_forwarded_ip_does_not_match
     server(
       itsi_rb: lambda do
         allow_list \
           allowed_patterns: ["^203\\.0\\.113\\.7$"],  # only allow this
           trusted_proxies: {
             "127.0.0.1" => { header: { name: "X-Forwarded-For" } }
           }
         get("/trusted") { |r| r.ok "nope" }
       end
     ) do
       # Send a forwarded IP that doesn't match the allow list
       res = get_resp("/trusted", { "X-Forwarded-For" => "192.0.2.55" })
       assert_equal "403", res.code
     end
   end

   def test_untrusted_proxy_ignores_forwarded_ip
     server(
       itsi_rb: lambda do
         allow_list \
           allowed_patterns: ["^203\\.0\\.113\\.7$"],  # client IP matches, but header is ignored
           trusted_proxies: {
             "10.0.0.1" => { header: { name: "X-Forwarded-For" } } # current proxy (127.0.0.1) is not trusted
           }
         get("/trusted") { |r| r.ok "never" }
       end
     ) do
       res = get_resp("/trusted", { "X-Forwarded-For" => "203.0.113.7" })
       # Since proxy is untrusted, header is ignored, and 127.0.0.1 is checked (not allowed)
       assert_equal "403", res.code
     end
   end
end
