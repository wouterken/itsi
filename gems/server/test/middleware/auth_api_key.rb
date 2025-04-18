require_relative "../helpers/test_helper"

class TestAuthApiKey < Minitest::Test

  # 1. Anonymous inline keys: valid secret in Authorization header
  def test_anonymous_inline_success
    server(
      itsi_rb: lambda do
        auth_api_key valid_keys: [
          Itsi.create_password_hash("supersecret", "sha256")
        ]
        get("/foo") {|r| r.ok "ok" }
      end
    ) do
      res = get_resp("/foo", { "Authorization" => "Bearer supersecret" })

      assert_equal "200", res.code
      assert_equal "ok", res.body
    end
  end

  # 2. Anonymous inline: missing token → 401
  def test_anonymous_inline_missing
    server(
      itsi_rb: lambda do
        auth_api_key valid_keys: [Itsi.create_password_hash("supersecret", "sha256")]
        get("/foo") {|r| r.ok "never" }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "401", res.code
    end
  end

  # 3. Identified inline keys: need both ID header and Bearer token
  def test_identified_inline_success
    key_id = "Key-1"
    server(
      itsi_rb: lambda do
        auth_api_key valid_keys: { "#{key_id}" => Itsi.create_password_hash("supersecret", "sha256") }
        get("/bar") {|r| r.ok "bar OK" }
      end
    ) do
      headers = {
        "X-Api-Key-Id" => key_id,
        "Authorization" => "Bearer supersecret"
      }
      res = get_resp("/bar", headers)
      assert_equal "200", res.code
      assert_equal "bar OK", res.body
    end
  end

  # 4. Identified inline: wrong ID → 401
  def test_identified_inline_wrong_id
    key_id = "Key-1"
    server(
      itsi_rb: lambda do
        auth_api_key valid_keys: { "#{key_id}" => Itsi.create_password_hash("supersecret", "sha256") }
        get("/bar") {|r| r.ok "never" }
      end
    ) do
      headers = { "X-Api-Key-Id" => "bad", "Authorization" => "Bearer supersecret" }
      res = get_resp("/bar", headers)
      assert_equal "401", res.code
    end
  end

  # 5. Custom token_source/query param
  def test_custom_token_source_query
    server(
      itsi_rb: lambda do
        auth_api_key \
          valid_keys: [Itsi.create_password_hash("supersecret", "sha256")],
          token_source: { query: "api_key" }
        get("/q") {|r| r.ok "qok" }
      end
    ) do
      res = get_resp("/q?api_key=supersecret")
      assert_equal "200", res.code
      assert_equal "qok", res.body
    end
  end

  # 6. Custom key_id_source/query param
  def test_custom_key_id_source_query
    server(
      itsi_rb: lambda do
        auth_api_key \
          valid_keys: { "#{@id1}" => Itsi.create_password_hash("supersecret", "sha256") },
          key_id_source: { query: "kid" }
        get("/q") {|r| r.ok "qok" }
      end
    ) do
      res = get_resp("/q?kid=#{@id1}", { "Authorization" => "Bearer supersecret" })
      assert_equal "200", res.code
      assert_equal "qok", res.body
    end
  end

  # 7. Custom error_response override
  def test_custom_error_response
    server(
      itsi_rb: lambda do
        auth_api_key \
          valid_keys: [Itsi.create_password_hash("supersecret", "sha256") ],
          error_response: { code: 403,
                            plaintext: { inline: "nope" },
                            default: "plaintext" }
        get("/x") {|r| r.ok "never" }
      end
    ) do
      res = get_resp("/x")
      assert_equal "403", res.code
      assert_equal "nope", res.body
    end
  end

  # 8. Scoped to a location path
  def test_scoped_to_location
    server(
      itsi_rb: lambda do
        location "/admin/*" do
          auth_api_key valid_keys: [Itsi.create_password_hash("supersecret", "sha256") ]
        end
        get("/admin/secret") {|r| r.ok "adm ok" }
        get("/public") {|r| r.ok "pub ok" }
      end
    ) do
      # Unprotected
      r1 = get_resp("/public")
      assert_equal "200", r1.code
      # Protected
      r2 = get_resp("/admin/secret")
      assert_equal "401", r2.code
      # Then with key
      r3 = get_resp("/admin/secret", { "Authorization" => "Bearer supersecret" })
      assert_equal "200", r3.code
    end
  end
end
