require_relative "../helpers/test_helper"

class TestWorkers < Minitest::Test
  def test_it_can_run_multiple_workers
    worker_count = 4

    server(
      itsi_rb: lambda do
        workers worker_count
        get("/foo") { |r| r.ok Process.pid }
      end
    ) do
      seen = Set.new
      Timeout.timeout(2) do
        loop do
          seen << get("foo")
          break if seen.size == worker_count
        end
      end
      assert_equal worker_count, seen.size
    end
  end
end
