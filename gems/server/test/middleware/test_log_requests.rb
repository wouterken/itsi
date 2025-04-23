require_relative "../helpers/test_helper"

class TestLogRequests < Minitest::Test
  # 1. before‑only logging
  def test_it_supports_logging_before_requests
    stdout, = capture_subprocess_io do
      server(
        itsi_rb: lambda do
          log_level :info
          log_requests \
            before: {
              level: "INFO",
              format: "[{request_id}] BEFORE {method} {path_and_query}"
            }
          get("/foo?bar=baz") { |r| r.ok "ok" }
        end
      ) do
        get_resp("/foo?bar=baz")
      end
    end
    server(
      itsi_rb: lambda do
        log_level :error
      end
    ){}

    # should emit something like "[a1b2c3] BEFORE GET /foo?bar=baz"
    assert_match(%r{\[[0-9a-f]+\] BEFORE GET /foo\?bar=baz}, stdout)
  end

  # 2. after‑only logging
  def test_it_supports_logging_after_requests
    stdout, = capture_subprocess_io do
      server(
        itsi_rb: lambda do
          log_level :info
          log_requests \
            after: {
              level: "INFO",
              format: "[{request_id}] AFTER {status} in {response_time}"
            }
          get("/foo") { |r| r.ok "ok" }
        end
      ) do
        get_resp("/foo")
      end

      server(
        itsi_rb: lambda do
          log_level :error
        end
      ){}

    end

    # should emit something like "[d4e5f6] AFTER 200 in 1.234ms"
    assert_match(/\[[0-9a-f]+\] AFTER 200 in \d+(?:\.\d+)?.?s/, stdout)
  end

  # 3. custom log‑level is honored
  def test_it_supports_configuring_log_level
    stdout, = capture_subprocess_io do
      server(
        itsi_rb: lambda do
          log_level :error
          log_requests \
            before: {
              level: "ERROR",
              format: "X"
            }
          get("/foo") { |r| r.ok "ok" }
        end
      ) do
        get_resp("/foo")
      end
    end

    # our line should begin with "ERROR" and then our literal "X"
    assert_match(/ERROR.*X/, stdout)
  end
end
