require_relative "../helpers/test_helper"

class TestMaxBody < Minitest::Test
  def test_max_body_enforced
    server(
      itsi_rb: lambda do
        max_body limit_bytes: 20
        post("/") { |r|
          r.ok "OK"
        }
      end
    ) do
      small = "a" * 10
      large = "b" * 100

      assert_equal "200", post("/", small).code
      assert_equal "413", post("/", large).code
    end
  end
end
