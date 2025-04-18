class TestHeaderInterpolation < Minitest::Test
  def test_request_header_interpolation_and_response_header_interpolation
    server(
      itsi_rb: lambda do
        # 1) Echo back incoming header "X-Auth-User" in a new request header "X-User"
        request_headers \
          additions: { "X-User" => ["{X-Auth-User}"] },
          removals:  []

        # 2) After the handler runs, take the request‑echoed "X-User" and also echo it into "X-User-Echo" in the response
        response_headers \
          additions: { "X-User-Echo" => ["{X-User}"] },
          removals:  []

        get("/echo") do |r|
          # Return the value of the new X-User header in the body for verification
          r.ok("Hello #{r.header("X-User").first}", headers: {"X-User" => [r.header("X-User").first]})
        end
      end
    ) do
      # Simulate a client sending X-Auth-User: alice
      req_headers = { "X-Auth-User" => "alice" }

      res = get_resp("/echo", req_headers)
      assert_equal "200", res.code

      # The request middleware should have created X-User == "alice",
      # and the handler returned it in the body:
      assert_equal "Hello alice", res.body

      # The response middleware should have added X-User-Echo == "alice"
      assert_equal "alice", res["X-User-Echo"]
    end
  end
end
