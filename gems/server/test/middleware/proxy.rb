require_relative "../helpers/test_helper"

class TestProxy < Minitest::Test

  def test_successful_forwarding

    backend_bind = free_bind
    server(
      itsi_rb: lambda do
        log_requests before: { format: "GET {path_and_query}", level: "INFO"}
        get("/foo") { |r|
          r.ok "backend success. #{r.query_params["bar"]}"
        }
      end,
      bind: backend_bind
    ) do
      server(
        itsi_rb: lambda do
          proxy \
            to: "#{backend_bind}{path_and_query}",
            backends: ["#{backend_bind}"],
            backend_priority: "round_robin",
            headers: {},
            verify_ssl: false,
            timeout: 30,
            tls_sni: false,
            error_response: "internal_server_error"
          get("/foo") { |r|
            r.ok "should not get here"
          }
        end
      ) do
        res = get_resp("/foo?bar=baz")
        # Expect that the proxy forwards the request to the backend.
        assert_equal "200", res.code, "Expected success status from backend"
        assert_equal "backend success. baz", res.body
      end
    end
  end

  # Test that an invalid target URL (i.e. an unparseable URL) yields an error response.
  def test_invalid_target_url_returns_error
    server(
      itsi_rb: lambda do
        proxy \
          to: "not_a_valid_url",
          backends: ["127.0.0.1:3001"],
          backend_priority: "round_robin",
          headers: {},
          verify_ssl: false,
          timeout: 30,
          tls_sni: false,
          error_response: "bad_gateway"
        get("/foo") { |r| r.ok "should not get here" }
      end
    ) do
      res = get_resp("/foo")
      assert_equal "502", res.code, "Expected error response code 502 for invalid URL"
    end
  end

  def test_overriding_headers

    backend_bind = free_bind
    server(
      itsi_rb: lambda do
        log_requests before: { format: "GET {path_and_query}", level: "INFO"}
        get("/header-test") do |r|
          # Return the incoming header value.
          r.ok r.header("X-Forwarded-For").first
        end
      end,
      bind: backend_bind
    ) do

      # Start proxy server.
      server(
        itsi_rb: lambda do
          proxy \
            to: "#{backend_bind}{path}{query}",
            backends: ["#{backend_bind}"],
            backend_priority: "round_robin",
            headers: { "X-Forwarded-For" => "{addr}" },
            verify_ssl: false,
            timeout: 30,
            tls_sni: false,
            error_response: "internal_server_error"
          get("/header-test") { |r| r.ok "should not get here" }
        end
      ) do
        res = get_resp("/header-test")
        # Expect that the overriding header "X-Forwarded-For" is set to the client’s address.
        assert_equal "127.0.0.1", res.body, "Expected the header to be set to the client address"
      end
    end
  end

  def test_proxy_with_static_to_only
     backend_bind = free_bind
     server(
       itsi_rb: lambda do
         get("/static") { |r| r.ok "static response" }
       end,
       bind: backend_bind
     ) do
       server(
         itsi_rb: lambda do
           proxy \
             to: "#{backend_bind}{path}{query}",
             backend_priority: "round_robin",
             headers: {},
             verify_ssl: false,
             timeout: 30,
             tls_sni: false,
             error_response: "internal_server_error"
           get("/static") { |r| r.ok "should not get here" }
         end
       ) do
         res = get_resp("/static")
         assert_equal "200", res.code, "Expected success status from static 'to' URL"
         assert_equal "static response", res.body
       end
     end
   end

   def test_proxy_with_backend_host_override
     backend_bind1 = free_bind
     backend_bind2 = free_bind
     # Start two dummy backends that respond with their Host header.
     thread1 = Thread.new do
       server(
         itsi_rb: lambda do
           get("/host-test") { |r| r.ok r.header("Host").first }
         end,
         bind: backend_bind1
       ){ sleep 1 }
     end
     thread2 = Thread.new do
       server(
         itsi_rb: lambda do
           get("/host-test") { |r| r.ok r.header("Host").first }
         end,
         bind: backend_bind2
       ){ sleep 1 }
     end

     sleep 0.1

     server(
       itsi_rb: lambda do
         proxy \
           to: "#{backend_bind1}{path}{query}",
           backends: ["#{backend_bind1}", "#{backend_bind2}"],
           backend_priority: "round_robin",
           headers: { "Host" => "custom.backend.example.com" },
           verify_ssl: false,
           timeout: 30,
           tls_sni: true,
           error_response: "internal_server_error"
         get("/host-test") { |r| r.ok "should not get here" }
       end
     ) do
       res = get_resp("/host-test")
       assert_equal "200", res.code, "Expected successful response with host override"
       assert_equal "custom.backend.example.com", res.body, "Expected the Host header to be overridden"
     end

     thread1.kill
     thread2.kill
   end

   def test_proxy_timeout
     backend_bind = free_bind
     # Start a backend server that sleeps for 1 second before responding.
     server(
       itsi_rb: lambda do
         get("/slow") do |r|
           sleep 1
           r.ok "delayed response"
         end
       end,
       bind: backend_bind
     ) do
       server(
         itsi_rb: lambda do
           # Set a very short timeout (0 seconds) to force a timeout.
           proxy \
             to: "#{backend_bind}{path}{query}",
             backends: ["#{backend_bind}"],
             backend_priority: "round_robin",
             headers: {},
             verify_ssl: false,
             timeout: 0,
             tls_sni: false,
             error_response: "gateway_timeout"
           get("/slow") { |r| r.ok "should not get here" }
         end
       ) do
         res = get_resp("/slow")
         assert_equal "504", res.code, "Expected 504 Gateway Timeout when backend response exceeds timeout"
       end
     end
   end

   def test_error_response_internal_server_error
     server(
       itsi_rb: lambda do
         proxy \
           to: "invalid_url",
           backends: ["127.0.0.1:3001"],
           backend_priority: "round_robin",
           headers: {},
           verify_ssl: false,
           timeout: 30,
           tls_sni: false,
           error_response: "internal_server_error"
         get("/foo") { |r| r.ok "should not get here" }
       end
     ) do
       res = get_resp("/foo")
       assert_equal "500", res.code, "Expected error response code 500 for internal_server_error"
     end
   end

   def test_error_response_not_found
     server(
       itsi_rb: lambda do
         proxy \
           to: "invalid_url",
           backends: ["127.0.0.1:3001"],
           backend_priority: "round_robin",
           headers: {},
           verify_ssl: false,
           timeout: 30,
           tls_sni: false,
           error_response: "not_found"
         get("/foo") { |r| r.ok "should not get here" }
       end
     ) do
       res = get_resp("/foo")
       assert_equal "404", res.code, "Expected error response code 404 for not_found"
     end
   end

   def test_error_response_unauthorized
     server(
       itsi_rb: lambda do
         proxy \
           to: "invalid_url",
           backends: ["127.0.0.1:3001"],
           backend_priority: "round_robin",
           headers: {},
           verify_ssl: false,
           timeout: 30,
           tls_sni: false,
           error_response: "unauthorized"
         get("/foo") { |r| r.ok "should not get here" }
       end
     ) do
       res = get_resp("/foo")
       assert_equal "401", res.code, "Expected error response code 401 for unauthorized"
     end
   end

   def test_error_response_forbidden
     server(
       itsi_rb: lambda do
         proxy \
           to: "invalid_url",
           backends: ["127.0.0.1:3001"],
           backend_priority: "round_robin",
           headers: {},
           verify_ssl: false,
           timeout: 30,
           tls_sni: false,
           error_response: "forbidden"
         get("/foo") { |r| r.ok "should not get here" }
       end
     ) do
       res = get_resp("/foo")
       assert_equal "403", res.code, "Expected error response code 403 for forbidden"
     end
   end

   def test_error_response_payload_too_large
     server(
       itsi_rb: lambda do
         proxy \
           to: "invalid_url",
           backends: ["127.0.0.1:3001"],
           backend_priority: "round_robin",
           headers: {},
           verify_ssl: false,
           timeout: 30,
           tls_sni: false,
           error_response: "payload_too_large"
         get("/foo") { |r| r.ok "should not get here" }
       end
     ) do
       res = get_resp("/foo")
       assert_equal "413", res.code, "Expected error response code 413 for payload_too_large"
     end
   end

   def test_error_response_too_many_requests
     server(
       itsi_rb: lambda do
         proxy \
           to: "invalid_url",
           backends: ["127.0.0.1:3001"],
           backend_priority: "round_robin",
           headers: {},
           verify_ssl: false,
           timeout: 30,
           tls_sni: false,
           error_response: "too_many_requests"
         get("/foo") { |r| r.ok "should not get here" }
       end
     ) do
       res = get_resp("/foo")
       assert_equal "429", res.code, "Expected error response code 429 for too_many_requests"
     end
   end

   def test_error_response_service_unavailable
     server(
       itsi_rb: lambda do
         proxy \
           to: "invalid_url",
           backends: ["127.0.0.1:3001"],
           backend_priority: "round_robin",
           headers: {},
           verify_ssl: false,
           timeout: 30,
           tls_sni: false,
           error_response: "service_unavailable"
         get("/foo") { |r| r.ok "should not get here" }
       end
     ) do
       res = get_resp("/foo")
       assert_equal "503", res.code, "Expected error response code 503 for service_unavailable"
     end
   end

   def test_error_response_gateway_timeout
     server(
       itsi_rb: lambda do
         proxy \
           to: "invalid_url",
           backends: ["127.0.0.1:3001"],
           backend_priority: "round_robin",
           headers: {},
           verify_ssl: false,
           timeout: 30,
           tls_sni: false,
           error_response: "gateway_timeout"
         get("/foo") { |r| r.ok "should not get here" }
       end
     ) do
       res = get_resp("/foo")
       assert_equal "504", res.code, "Expected error response code 504 for gateway_timeout"
    end
  end

  def test_failover_behavior
    # Obtain two free bind addresses for backend servers.
    backend1_bind = free_bind
    backend2_bind = free_bind

    backend2_server = Itsi::Server.start_in_background_thread(binds: [backend2_bind]) do
      get("/failover") { |r| r.ok "backend2" }
    end

    # backend2_server.stop

    # # Allow the backend servers to start.
    sleep 0.2


    # Start the proxy server with an ordered backend selection.
    server(
      cleanup: false,
      itsi_rb: lambda do
        proxy \
          to: "http://proxied_host.com{path}{query}",
          backends:  [backend1_bind[/\/\/(.*)/,1], backend2_bind[/\/\/(.*)/,1]],
          backend_priority: "ordered",
          headers: {},
          verify_ssl: false,
          timeout: 30,
          tls_sni: false,
          error_response: "internal_server_error"
        get("/failover") { |r| r.ok "should not get here" }
      end
    ) do


      res = get_resp("/failover")
      assert_equal "200", res.code, "Expected success when fallback backend is available"
      assert_equal "backend2", res.body, "Expected response from backend2"

      backend1_server = Itsi::Server.start_in_background_thread(binds: [backend1_bind]) do
        get("/failover") { |r| r.ok "backend1" }
      end

      sleep 1

      res = get_resp("/failover")
      assert_equal "200", res.code, "Expected reversion when primary becomes available"
      assert_equal "backend1", res.body, "Expected response from backend1 with ordered priority"
    end

    Itsi::Server.stop_background_threads
  end
end
