require_relative "../helpers/test_helper"

class TestUnmatchedRequest < Minitest::Test
  def test_connect_request_without_matching_stack_does_not_stop_listener
    server(
      itsi_rb: lambda do
        get("/ok") { |r| r.respond("ok") }
      end
    ) do
      response = raw_http("CONNECT google.com:443 HTTP/1.1\r\nHost: google.com:443\r\n\r\n")

      assert_match(/\AHTTP\/\d(?:\.\d)? 404\b/, response)
      assert_equal "ok", get("/ok")
    end
  end

  def test_origin_form_request_without_leading_slash_does_not_stop_listener
    server(
      itsi_rb: lambda do
        get("/ok") { |r| r.respond("ok") }
      end
    ) do
      response = raw_http("GET default.asp HTTP/1.1\r\nHost: example.com\r\n\r\n")

      assert_match(/\AHTTP\/\d(?:\.\d)? 404\b/, response)
      assert_equal "ok", get("/ok")
    end
  end
end
