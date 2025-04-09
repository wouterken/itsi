require_relative "../helpers/test_helper"

class TestThreads < Minitest::Test

  def test_it_can_run_multiple_threads
    thread_count = 4
    server(
      itsi_rb: lambda do
        threads thread_count
        get("/foo"){|r| r.ok Thread.current.object_id}
      end) do
        assert_equal thread_count, 30.times.map { |i| get("/foo") }.uniq.size
    end
  end
end
