require_relative "../../helpers/test_helper"
require_relative "test_service_impl"

class TestGrpc < Minitest::Test
  Stub = Test::TestService::Stub

  def new_stub(uri)
    address = uri.to_s[/\/\/(.*)/,1]    # e.g. "127.0.0.1:12345"
    channel = GRPC::Core::Channel.new(address,
                                      {},              # channel_args hash
                                      :this_channel_is_insecure,
    )
    stub = Stub.new(nil,
                                 nil,
                                 channel_override: channel)
  end

  def test_unary_echo
    server(itsi_rb: lambda do
      grpc TestServiceImpl.new
    end) do
      stub = new_stub(@uri)
      req = Test::EchoRequest.new(message: "hello")
      res = stub.unary_echo(req)
      assert_equal "hello", res.message
    end
  end

  def test_client_stream
    server(itsi_rb: lambda do
      grpc TestServiceImpl.new
    end) do
      stub = new_stub(@uri)
      # send a few messages in one stream
      requests = %w[a b c].map { |m| Test::StreamRequest.new(message: m) }
      res = stub.client_stream(requests.each)
      assert_equal %w[a b c], res.messages
    end
  end

  def test_server_stream
    server(itsi_rb: lambda do
      grpc TestServiceImpl.new
    end) do
      stub = new_stub(@uri)
      req = Test::EchoRequest.new(message: "xy")
      # expect two StreamResponse frames, one per character
      chars = stub.server_stream(req).map(&:messages).flatten
      assert_equal ["x", "y"], chars
    end
  end

  def test_bidi_stream
    server(itsi_rb: lambda do
      grpc TestServiceImpl.new
    end) do
      stub = new_stub(@uri)
      inputs = %w[test1 test2].map { |m| Test::EchoRequest.new(message: m) }
      # each response uppercases its incoming message
      results = stub.bidi_stream(inputs.each).map(&:message)
      assert_equal ["TEST1", "TEST2"], results
    end
  end

  def test_empty_streams
    server(itsi_rb: lambda do
      grpc TestServiceImpl.new
    end) do
      stub = new_stub(@uri)

      # empty client‐streaming
      res = stub.client_stream([].each)
      assert_equal [], res.messages

      # empty server‐streaming
      empty_server = stub.server_stream(Test::EchoRequest.new(message: "")).to_a
      assert empty_server.empty?

      # empty bidi‐streaming
      empty_bidi = stub.bidi_stream([].each).to_a
      assert empty_bidi.empty?
    end
  end


  def test_unary_echo_json
    server(itsi_rb: lambda do
      grpc TestServiceImpl.new
    end) do
      payload = { "message" => "hello" }.to_json
      res = post("test.TestService/UnaryEcho", payload, { "Content-Type" => "application/json" })
      json = JSON.parse(res.body)
      assert_equal 200, res.code.to_i
      assert_equal "hello", json["message"]
    end
  end

  def test_client_stream_json
    server(itsi_rb: lambda do
      grpc TestServiceImpl.new
    end) do
      input = [{"message" => "a"}, {"message" => "b"}, {"message" => "c"}]
      res = post("test.TestService/ClientStream", input.to_json, { "Content-Type" => "application/json" })
      json = JSON.parse(res.body)
      assert_equal 200, res.code.to_i
      assert_equal %w[a b c], json["messages"]
    end
  end

  def test_server_stream_json
    server(itsi_rb: lambda do
      grpc TestServiceImpl.new
    end) do
      payload = { "message" => "xy" }.to_json
      res = post("test.TestService/ServerStream", payload, { "Content-Type" => "application/json" })
      arr = JSON.parse(res.body)
      # service streams back one JSON object per character
      messages = arr.flat_map { |frame| frame["messages"] }
      assert_equal 200, res.code.to_i
      assert_equal ["x","y"], messages
    end
  end

  def test_bidi_stream_json
    server(itsi_rb: lambda do
      grpc TestServiceImpl.new
    end) do
      inputs = [{"message":"foo"}, {"message":"Bar"}]
      res = post("test.TestService/BidiStream", inputs.to_json, { "Content-Type" => "application/json" })
      arr = JSON.parse(res.body)
      # bidi uppercases each incoming message
      results = arr.map { |frame| frame["message"] }
      assert_equal 200, res.code.to_i
      assert_equal ["FOO","BAR"], results
    end
  end

  def test_empty_streams_json
    server(itsi_rb: lambda do
      grpc TestServiceImpl.new
    end) do
      # empty client-stream → still JSON object with empty messages
      res1 = post("test.TestService/ClientStream", "[]", { "Content-Type" => "application/json" })
      json1 = JSON.parse(res1.body)
      assert_nil json1["messages"]

      # empty server-stream
      res2 = post("test.TestService/ServerStream", { "message" => "" }.to_json, { "Content-Type" => "application/json" })
      arr2 = JSON.parse(res2.body)
      assert arr2.empty?

      # empty bidi-stream
      res3 = post("test.TestService/BidiStream", "[]", { "Content-Type" => "application/json" })
      arr3 = JSON.parse(res3.body)
      assert arr3.empty?
    end
  end
end
