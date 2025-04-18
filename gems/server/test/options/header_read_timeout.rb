require_relative "../helpers/test_helper"

class TestHeaderReadTimeout < Minitest::Test
  def test_header_timeout_enforced
    server(
      itsi_rb: lambda do
        header_read_timeout 0.1
        get("/") { |r| r.ok "hi" }
      end
    ) do
      socket = TCPSocket.new("127.0.0.1", @uri.port)
      socket.write("GET / HTTP/1.1\r\nHost: localhost\r\n") # don’t send \r\n\r\n

      sleep 0.2 # exceed header timeout
      socket.write("\r\n\r\n")
      response = socket.read
    rescue StandardError => e
      assert_equal e.class, Errno::ECONNRESET
    ensure
      socket&.close
    end
  end
end
