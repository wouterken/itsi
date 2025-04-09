require_relative "../helpers/test_helper"

class TestLogRequests < Minitest::Test

  def test_it_supports_logging_before_requests
    server(
      itsi_rb: lambda do
        log_requests after: {
          format: "[{request_id}] {method} {path_and_query} - {addr}",

        }
        get("/foo"){|r| r.ok "Ok"}
      end
    ) do
        assert_equal thread_count, 30.times.map { |i| get("/foo") }.uniq.size
    end
  end

  def test_it_supports_logging_after_requests
    server(
      itsi_rb: lambda do
        log_requests after: {
          format: "[{request_id}] {method} {path_and_query} - {addr}",

        }
        get("/foo"){|r| r.ok "Ok"}
      end
    ) do
        assert_equal thread_count, 30.times.map { |i| get("/foo") }.uniq.size
    end
  end

  def test_it_supports_configuring_log_level
    server(
      itsi_rb: lambda do
        log_requests before: {
          level: :info
        }
        get("/foo"){|r| r.ok "Ok"}
      end
    ) do
        assert_equal thread_count, 30.times.map { |i| get("/foo") }.uniq.size
    end

    server(
      itsi_rb: lambda do
        log_requests before: {
          level: :debug
        }
        get("/foo"){|r| r.ok "Ok"}
      end
    ) do
        assert_equal thread_count, 30.times.map { |i| get("/foo") }.uniq.size
    end
  end

end
