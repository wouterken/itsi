require_relative "../helpers/test_helper"
require "jwt"
require "openssl"
require "securerandom"
require "base64"

class TestAuthJwt < Minitest::Test
  ALGORITHMS = {
    "HS256" => -> {
      secret = Base64.strict_encode64(SecureRandom.random_bytes(32))
      verifier = secret
      signer  = Base64.decode64(secret)
      [verifier, signer]
    },
    "HS384" => -> {
      secret = Base64.strict_encode64(SecureRandom.random_bytes(48))
      [secret, Base64.decode64(secret)]
    },
    "HS512" => -> {
      secret = Base64.strict_encode64(SecureRandom.random_bytes(64))
      [secret, Base64.decode64(secret)]
    },
    "RS256" => -> {
      rsa = OpenSSL::PKey::RSA.new(2048)
      [rsa.public_key.to_pem, rsa]
    },
    "RS384" => -> {
      rsa = OpenSSL::PKey::RSA.new(2048)
      [rsa.public_key.to_pem, rsa]
    },
    "RS512" => -> {
      rsa = OpenSSL::PKey::RSA.new(2048)
      [rsa.public_key.to_pem, rsa]
    },
    "PS256" => -> {
      rsa = OpenSSL::PKey::RSA.new(2048)
      [rsa.public_key.to_pem, rsa]
    },
    "PS384" => -> {
      rsa = OpenSSL::PKey::RSA.new(2048)
      [rsa.public_key.to_pem, rsa]
    },
    "PS512" => -> {
      rsa = OpenSSL::PKey::RSA.new(2048)
      [rsa.public_key.to_pem, rsa]
    },
    "ES256" => -> {
      private_key = OpenSSL::PKey::EC.generate("prime256v1")
      asn1 = OpenSSL::ASN1::Sequence(
          [
            OpenSSL::ASN1::Sequence(
              [
                OpenSSL::ASN1::ObjectId("id-ecPublicKey"),
                OpenSSL::ASN1::ObjectId(private_key.public_key.group.curve_name)
              ]
            ),
            OpenSSL::ASN1::BitString(private_key.public_key.to_octet_string(:uncompressed))
          ]
        )
      public_key = OpenSSL::PKey::EC.new(asn1.to_der)
      [public_key.to_pem, private_key]
    },
    "ES384" => -> {
      private_key = OpenSSL::PKey::EC.generate("secp384r1")
      asn1 = OpenSSL::ASN1::Sequence(
          [
            OpenSSL::ASN1::Sequence(
              [
                OpenSSL::ASN1::ObjectId("id-ecPublicKey"),
                OpenSSL::ASN1::ObjectId(private_key.public_key.group.curve_name)
              ]
            ),
            OpenSSL::ASN1::BitString(private_key.public_key.to_octet_string(:uncompressed))
          ]
        )
      public_key = OpenSSL::PKey::EC.new(asn1.to_der)
      [public_key.to_pem, private_key]
    }
  }

  # 1. Missing token → 401
  def test_missing_token
    server(
      itsi_rb: lambda do
        auth_jwt verifiers: { "HS256" => ["Zm9v"] }
        get("/j") { |r| r.ok "j" }
      end
    ) do
      res = get_resp("/j")
      assert_equal "401", res.code
    end
  end

  # 2. Invalid token → 401
  def test_invalid_token
    server(
      itsi_rb: lambda do
        auth_jwt verifiers: { "HS256" => ["Zm9v"] }
        get("/j") { |r| r.ok "j" }
      end
    ) do
      res = get_resp("/j", { "Authorization" => "Bearer invalid.token" })
      assert_equal "401", res.code
    end

  end

  # Dynamically define one test per algorithm
  ALGORITHMS.each do |alg, gen|
    define_method("test_#{alg.downcase}_success") do
      verifier, signer = gen.call
      # build a minimal payload
      payload = { "sub" => "user", "aud" => "aud1", "iss" => "iss1", "iat" => Time.now.to_i, exp: Time.now.to_i + 10 }
      token = JWT.encode(payload, signer, alg)
      server(
        itsi_rb: lambda do
          auth_jwt verifiers: { alg => [verifier] }
          get("/#{alg.downcase}") { |r| r.ok alg }
        end
      ) do
        res = get_resp("/#{alg.downcase}", { "Authorization" => "Bearer #{token}" })
        assert_equal "200", res.code, "#{alg} should verify successfully"
        assert_equal alg, res.body
      end
    end
  end

  # 5. Audience restriction
  def test_audience_restriction
    verifier, signer = ALGORITHMS["HS256"].call
    payload = { "aud" => "good", exp: Time.now.to_i + 10 }
    token   = JWT.encode(payload, signer, "HS256")
    server(
      itsi_rb: lambda do
        auth_jwt verifiers: { "HS256" => [verifier] }, audiences: ["good"]
        get("/a") { |r| r.ok "ok" }
      end
    ) do
      res1 = get_resp("/a", { "Authorization" => "Bearer #{token}" })
      assert_equal "200", res1.code
      # wrong audience
      bad = JWT.encode({ "aud" => "bad" }, signer, "HS256")
      res2 = get_resp("/a", { "Authorization" => "Bearer #{bad}" })
      assert_equal "401", res2.code
    end
  end

  # 6. Subject & issuer restrictions
  def test_subject_and_issuer
    verifier, signer = ALGORITHMS["HS256"].call
    payload = { "sub" => "mysub", "iss" => "myiss", exp: Time.now.to_i + 10 }
    token   = JWT.encode(payload, signer, "HS256")
    server(
      itsi_rb: lambda do
        auth_jwt verifiers: { "HS256" => [verifier] },
                 subjects: ["mysub"], issuers: ["myiss"]
        get("/si") { |r| r.ok "si" }
      end
    ) do
      res = get_resp("/si", { "Authorization" => "Bearer #{token}" })
      assert_equal "200", res.code
    end
  end

  # 7. Custom token_source query
  def test_custom_token_source_query
    verifier, signer = ALGORITHMS["HS256"].call
    token = JWT.encode({exp: Time.now.to_i + 10}, signer, "HS256")
    server(
      itsi_rb: lambda do
        auth_jwt verifiers: { "HS256" => [verifier] }, token_source: { query: "jwt" }
        get("/") { |r| r.ok "root" }
      end
    ) do
      res = get_resp("/?jwt=#{token}")
      assert_equal "200", res.code
    end
  end

  # 8. Leeway works for 'exp' skew
  def test_leeway_exp
    verifier, signer = ALGORITHMS["HS256"].call
    expired_iat = Time.now.to_i - 120
    token = JWT.encode({ "exp" => expired_iat }, signer, "HS256")
    server(
      itsi_rb: lambda do
        auth_jwt verifiers: { "HS256" => [verifier] }, leeway: 300
        get("/") { |r| r.ok "ok" }
      end
    ) do
      res = get_resp("/", { "Authorization" => "Bearer #{token}" })
      assert_equal "200", res.code
    end
  end

  # 9. Scoped to location
  def test_scoped_to_location
    verifier, signer = ALGORITHMS["HS256"].call
    token = JWT.encode({exp: Time.now.to_i + 10}, signer, "HS256")
    server(
      itsi_rb: lambda do
        location "/api/*" do
          auth_jwt verifiers: { "HS256" => [verifier] }
          get("x") { |r| r.ok "x" }
        end
        get("/public") { |r| r.ok "p" }
      end
    ) do
      assert_equal "p", get_resp("/public").body
      assert_equal "401", get_resp("/api/x").code
      assert_equal "x", get_resp("/api/x", { "Authorization" => "Bearer #{token}" }).body
    end
  end
end
