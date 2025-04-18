# test_static_response.rb
require_relative "../helpers/test_helper"

class TestStaticResponse < Minitest::Test
  def test_basic_response_code_and_body
    server(
      itsi_rb: lambda do
        static_response \
          code: 204,
          headers: [],
          body: ""
      end
    ) do
      res = get_resp("/any")
      assert_equal "204", res.code
      assert_nil   res.body   # 204 should have no body
    end
  end

  def test_custom_headers_and_body
    server(
      itsi_rb: lambda do
        static_response \
          code: 418,
          headers: [
            ["Content-Type","text/teapot"],
            ["X-Test","value"]
          ],
          body:    "I'm a teapot"
      end
    ) do
      res = get_resp("/brew")
      assert_equal "418", res.code
      assert_equal "text/teapot", res["Content-Type"]
      assert_equal "value",        res["X-Test"]
      assert_equal "I'm a teapot", res.body
    end
  end

  def test_binary_body
    data = [0x00,0xFF,0x7F]
    server(
      itsi_rb: lambda do
        static_response \
          code: 200,
          headers: [["Content-Type","application/octet-stream"]],
          body:    data.pack("C*")
      end
    ) do
      res = get_resp("/bin")
      # Ensure raw bytes round‑trip
      assert_equal data.pack("C*"), res.body
      assert_equal "application/octet-stream", res["Content-Type"]
    end
  end
end
