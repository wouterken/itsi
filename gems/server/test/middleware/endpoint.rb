require_relative "../helpers/test_helper"
require "json"

MySchema = { _required: %i[name], name: String }
PingSchema = { _required: %i[status], status: String }


class TestEndpoint < Minitest::Test
  # 1. Basic inline GET endpoint
  def test_basic_get
    server(itsi_rb: lambda do
      endpoint "/hello" do |req|
        req.ok "world"
      end
    end) do
      res = get_resp("/hello")
      assert_equal "200", res.code
      assert_equal "world", res.body
    end
  end

  # 2. GET-only wrapper rejects POST
  def test_get_wrapper_rejects_post
    server(itsi_rb: lambda do
      get "/onlyget" do |req|
        req.ok "got"
      end
    end) do
      assert_equal "got", get("/onlyget")
      post_res = post("/onlyget", "")
      refute_equal post_res.body, "got"
    end
  end

  # 3. Trailing slash handling: both /path and /path/ map
  def test_trailing_slash_equivalence
    server(itsi_rb: lambda do
      endpoint "/users" do |req|
        req.ok req.path
      end
    end) do
      res1 = get_resp("/users")
      assert_equal "200", res1.code
      assert_equal "/users", res1.body

      res2 = get_resp("/users/")
      assert_equal "200", res2.code
      assert_equal "/users/", res2.body
    end
  end

  # 4. Empty‑path endpoint: wildcard match all paths
  def test_empty_path_wildcard
    server(itsi_rb: lambda do
      endpoint do |req|
        req.ok req.path
      end
    end) do
      %w[/ /foo /foo/bar].each do |path|
        res = get_resp(path)
        assert_equal "200", res.code
        assert_equal path, res.body
      end
    end
  end

  # 5. JSON body parsing into params
  def test_json_body_parsing
    server(itsi_rb: lambda do
      endpoint "/echo_json" do |req, params|
        req.ok JSON.generate(params)
      end
    end) do
      payload = { "a" => 1, "b" => 2 }
      res = post(
        "/echo_json",
        JSON.generate(payload),
        "Content-Type" => "application/json"
      )
      assert_equal "200", res.code
      assert_equal payload, JSON.parse(res.body)
    end
  end

  # 6. Form‑encoded body parsing
  def test_form_urlencoded_parsing
    server(itsi_rb: lambda do
      endpoint "/echo_form" do |req, params|
        req.ok "#{params['x']}-#{params['y']}"
      end
    end) do
      res = post(
        "/echo_form",
        "x=foo&y=bar",
        "Content-Type" => "application/x-www-form-urlencoded"
      )
      assert_equal "200", res.code
      assert_equal "foo-bar", res.body
    end
  end

  # 7. Streaming via low‑level response object
  def test_streaming_response
    server(itsi_rb: lambda do
      endpoint "/stream" do |req|
        stream = req.response
        stream << "A"
        stream << "B"
        stream << "C"
        stream.close
      end
    end) do
      res = get_resp("/stream")
      assert_equal "200", res.code
      assert_equal "ABC", res.body
    end
  end

  # 8. Exceptions inside handler produce 500
  def test_internal_error_returns_500
    server(itsi_rb: lambda do
      endpoint "/boom" do |_req|
        raise "test crash"
      end
    end) do
      res = get_resp("/boom")
      assert_equal "500", res.code
    end
  end

  # 9. Controller method dispatch via symbol
  def test_controller_symbol_dispatch
    server(itsi_rb: lambda do
      # define handler method in top‑level scope
      def foo_handler(req)
        req.ok "from_symbol"
      end
      endpoint "/sym", :foo_handler
    end) do
      res = get_resp("/sym")
      assert_equal "200", res.code
      assert_equal "from_symbol", res.body
    end
  end

  # 10. Schema enforcement rejects bad JSON
  def test_schema_validation_failure
    server(itsi_rb: lambda do
      # declare a schema and use typed params
      endpoint "/typed" do |req, params: MySchema|
        req.ok params["name"]
      end
    end) do
      # missing "name"
      res = post(
        "/typed",
        "{}",
        "Content-Type" => "application/json"
      )
      assert_equal "400", res.code
      assert_match /Validation failed/, res.body
    end
  end

  # 11. Response‑format schema enforcement
  def test_response_schema_enforcement
    server(itsi_rb: lambda do
      endpoint "/ping" do |req, _params, response_format: PingSchema|
        req.ok json: { "wrong" => 123 }, as: response_format
      end
    end) do
      res = get_resp("/ping")
      assert_equal "400", res.code
      assert_match /Validation failed/, res.body
    end
  end

  # 12. Unrestricted HTTP method on endpoint (no wrapper)
  def test_unrestricted_endpoint_all_methods
    server(itsi_rb: lambda do
      endpoint "/all" do |req|
        req.ok req.request_method
      end
    end) do
      %w[POST PUT PATCH DELETE].each do |m|
        resp = send(m.downcase, "/all")
        assert_equal "200", resp.code
        assert_equal m, resp.body
      end
      resp = get_resp("/all")
      assert_equal "200", resp.code
      assert_equal "GET", resp.body
    end
  end

  def test_head_on_get_wrapper
    server(itsi_rb: lambda do
      get "/resource" do |req|
        req.respond body: "payload", status: 200, headers: { "X-Foo" => "bar" }
      end
    end) do
      res = head("/resource")
      assert_equal "200", res.code
      assert_equal "bar", res["X-Foo"]
      assert_empty res.body.to_s
    end
  end

  # 14. POST wrapper accepts POST only
  def test_post_wrapper
    server(itsi_rb: lambda do
      post "/submit" do |req|
        req.ok "posted"
      end
      endpoint do |req|
        req.not_found ""
      end
    end) do
      assert_equal "posted", post("/submit", "").body
      assert_equal "404", get_resp("/submit").code
      assert_equal "404", head("/submit").code
      assert_equal "404", options("/submit").code
    end
  end

  # 15. PUT, PATCH, DELETE wrappers
  %w[put patch delete].each do |verb|
    define_method("test_#{verb}_wrapper") do
      server(itsi_rb: lambda do
        send(verb, "/thing") do |req|
          req.ok req.request_method
        end
        endpoint do |req|
          req.not_found ""
        end
      end) do
        # correct method
        send("#{verb}", "/thing")
        # mismatched methods
        %w[POST OPTIONS HEAD].each do |bad|
          res = send("#{bad.downcase}", "/thing")
          assert_equal "404", res.code, "#{verb.upcase} wrapper should 404 on #{bad}"
        end
      end
    end
  end

  # 16. OPTIONS on unrestricted endpoint yields 200
  def test_options_on_unrestricted
    server(itsi_rb: lambda do
      endpoint "/open" do |req|
        req.ok "ok"
      end
    end) do
      res = options("/open")
      assert_equal "200", res.code
      assert_equal "ok", res.body
    end
  end

  # 17. HEAD on streaming endpoint
  def test_head_on_streaming_endpoint
    server(itsi_rb: lambda do
      endpoint "/stream" do |req|
        s = req.response
        s << "XYZ"
        s.close
      end
    end) do
      res = head("/stream")
      assert_equal "200", res.code
      assert_empty res.body.to_s
    end
  end

  # 18. Wildcard segment matching
  def test_wildcard_endpoint
    server(itsi_rb: lambda do
      endpoint "/api/*" do |req|
        req.ok req.path
      end
    end) do
      ["/api/foo", "/api/foo/bar", "/api/"].each do |path|
        res = get_resp(path)
        assert_equal path, res.body
      end
    end
  end

  # 19. Multiple endpoints same path, different verbs
  def test_same_path_different_verb
    server(itsi_rb: lambda do
      get "/duo" do |req| req.ok "g" end
      post "/duo" do |req| req.ok "p" end
    end) do
      assert_equal "g", get("/duo")
      assert_equal "p", post("/duo", "").body
    end
  end
end
