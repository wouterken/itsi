require_relative "../helpers/test_helper"

class TestRequestTimeout < Minitest::Test
  def test_request_timeout_applies
    server(
      itsi_rb: lambda do
        request_timeout 0.1
        get("/") { |r| sleep 0.2; r.ok "late" }
      end
    ) do
      res = get_resp("/")
      assert_equal "504", res.code
    end
  end
end
