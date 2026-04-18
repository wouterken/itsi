require_relative "../helpers/test_helper"

class TestTlsHandshakeTimeout < Minitest::Test
  def test_incomplete_tls_handshake_does_not_block_listener_indefinitely
    server(
      protocol: "https",
      itsi_rb: lambda do
        header_read_timeout 5.0
        get("/ok") { |r| r.ok "ok" }
      end
    ) do
      socket = TCPSocket.new("127.0.0.1", @uri.port)

      assert_equal "ok", get("/ok")
    ensure
      socket&.close
    end
  end
end
