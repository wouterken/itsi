require_relative "../helpers/test_helper"

class TestWorkers < Minitest::Test

  def test_it_can_run_multiple_workers
    worker_count = 4

    server(
      itsi_rb: lambda do

        workers worker_count
        get("/foo"){|r| r.ok Process.pid}

      end) do
        assert_equal worker_count, 30.times.map { |i| get("/foo") }.uniq.size
    end
  end
end
