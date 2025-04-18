require_relative "../helpers/test_helper"

class TestAuthBasic < Minitest::Test


  # 1. Inline credentials success
  def test_inline_success
    server(
      itsi_rb: lambda do
        auth_basic realm: "Admin", credential_pairs: { "alice" => Itsi.create_password_hash("wonderland", "sha256") }
        get("/secure") {|r| r.ok "hello" }
      end
    ) do
      hdr = { "Authorization" => "Basic #{["alice:wonderland"].pack("m0")}" }
      res = get_resp("/secure", hdr)
      assert_equal "200", res.code
      assert_equal "hello", res.body
    end
  end

  # 2. Missing credentials → 401 + WWW-Authenticate header
  def test_missing_credentials
    server(
      itsi_rb: lambda do
        auth_basic realm: "Admin", credential_pairs: { "alice" => Itsi.create_password_hash("wonderland", "sha256") }
        get("/secure") {|r| r.ok "never" }
      end
    ) do
      res = get_resp("/secure")
      assert_equal "401", res.code
      assert_match /Basic realm="Admin"/, res["WWW-Authenticate"]
    end
  end

  # 3. Invalid credentials → 401
  def test_invalid_credentials
    server(
      itsi_rb: lambda do
        auth_basic realm: "Area", credential_pairs: { "alice" => Itsi.create_password_hash("wonderland", "sha256") }
        get("/secure") {|r| r.ok "never" }
      end
    ) do
      bad = ["bob:wrong"].pack("m0")
      res = get_resp("/secure", { "Authorization" => "Basic #{bad}" })
      assert_equal "401", res.code
    end
  end

  # 4. Load from credentials_file
  def test_credentials_file
    Dir.mktmpdir do |dir|
      Dir.chdir("#{dir}") do
        path = File.join(dir, ".itsi-credentials")
        File.write(path, "alice:#{Itsi.create_password_hash("wonderland", "sha256")}\n")
        server(
          itsi_rb: lambda do
            auth_basic  # default will load .itsi-credentials
            get("/a") {|r| r.ok "ok" }
          end
        ) do
          hdr = { "Authorization" => "Basic #{["alice:wonderland"].pack("m0")}" }
          res = get_resp("/a", hdr)
          assert_equal "200", res.code
        end
      end
    end
  end

  # 5. Apply to sub-path only
  def test_scoped_to_location
    server(
      itsi_rb: lambda do
        location "/admin/*" do
          auth_basic realm: "Adm", credential_pairs: { "alice" => Itsi.create_password_hash("wonderland", "sha256") }
        end
        get("/admin/yes") {|r| r.ok "y"}
        get("/no")        {|r| r.ok "n"}
      end
    ) do
      # outside
      assert_equal "n", get_resp("/no").body
      # inside, need auth
      res1 = get_resp("/admin/yes")
      assert_equal "401", res1.code
      # then with creds
      hdr = { "Authorization" => "Basic #{["alice:wonderland"].pack("m0")}" }
      res2 = get_resp("/admin/yes", hdr)
      assert_equal "200", res2.code
    end
  end
end
