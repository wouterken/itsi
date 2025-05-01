require_relative "../helpers/test_helper"

class TestRubyThreadRequestBacklogSize < Minitest::Test

  def test_ruby_thread_request_backlog_size
    server(
      itsi_rb: lambda do
        threads 1
        workers 1
        ruby_thread_request_backlog_size 1
        get("/foo"){|r| sleep 0.1; r.ok "ok" }
      end) do
      responses = 10.times.map{ Thread.new{ get_resp("/foo") } }.map(&:value)

      assert responses.map(&:code).include?("200")
      assert responses.map(&:code).include?("503")
    end
  end

  def test_ruby_thread_request_backlog_size_default
    server(
      itsi_rb: lambda do
        threads 1
        workers 1
        # ruby_thread_request_backlog_size 1 - Disabled. Should revert to more generous default
        get("/foo"){|r| sleep 0.01; r.ok "ok" }
      end) do
      responses = 29.times.map{ Thread.new{ get_resp("/foo") } }.map(&:value)

      assert_equal "ok", responses.first.body
      assert_equal "200", responses.first.code
      assert_equal "ok", responses.last.body
      assert_equal "200", responses.last.code
    end
  end
end
