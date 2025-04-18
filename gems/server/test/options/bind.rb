require_relative "../helpers/test_helper"

class TestBind < Minitest::Test
  def test_http
    server(
      app: lambda do |env|
      [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
    end) do
      assert_equal "Hello, World!", get("/")
    end
  end


  def test_https
    server(app: lambda do |env|
      [200, { "Content-Type" => "text/plain" }, ["Hello, HTTPS!"]]
    end, protocol: "https") do |uri|
      response = Net::HTTP.start(uri.hostname, uri.port, use_ssl: true,
                                                          verify_mode: OpenSSL::SSL::VERIFY_NONE) do |http|
        http.request(Net::HTTP::Get.new("/"))
      end
      assert_equal "200", response.code
      assert_equal "Hello, HTTPS!", response.body
    end
  end

  def test_unix_socket_http
    server(
      bind: free_bind("http", unix_socket: true),
      app: lambda do |env|
      [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
    end) do
      assert_equal "Hello, World!", get("/")
    end
  end

  def test_unix_socket_https
    server(
      bind: free_bind("http", unix_socket: true),
      app: lambda do |env|
      [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
    end) do
      assert_equal "Hello, World!", get("/")
    end
  end

end
