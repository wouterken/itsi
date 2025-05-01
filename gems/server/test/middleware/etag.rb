require_relative "../helpers/test_helper"

class TestETag < Minitest::Test
  def test_strong_etag_added
    server(
      itsi_rb: lambda do
        etag type: "strong", algorithm: "sha256", min_body_size: 0
        get("/foo") { |r| r.ok "etag-body" }
      end
    ) do
      res = get_resp("/foo")
      assert res["ETag"].start_with?("\"")
      refute res["ETag"].start_with?("W/")
    end
  end

  def test_weak_etag_added_with_md5
    server(
      itsi_rb: lambda do
        etag type: "weak", algorithm: "md5", min_body_size: 0
        get("/foo") { |r| r.ok "etag-weak-md5" }
      end
    ) do
      res = get_resp("/foo")
      assert res["ETag"].start_with?("W/\"")
    end
  end

  def test_etag_not_added_below_min_body_size
    server(
      itsi_rb: lambda do
        etag min_body_size: 50
        get("/foo") { |r| r.ok "short" }
      end
    ) do
      res = get_resp("/foo")
      refute res.key?("ETag")
    end
  end

  def test_etag_responds_with_304_if_none_match
    body = "etag-content"
    server(
      itsi_rb: lambda do
        etag type: "strong", min_body_size: 0
        get("/foo") { |r| r.ok body }
      end
    ) do
      first = get_resp("/foo")
      etag = first["ETag"]
      assert etag

      second = get_resp("/foo", { "If-None-Match" => etag })
      assert_equal "304", second.code
      refute second.body && second.body.size > 0
    end
  end

  def test_etag_ignored_for_streaming_body
    server(
      itsi_rb: lambda do
        etag min_body_size: 0
        get("/stream") do |req|
          r = req.response
          r << "streaming-1"
          r << "streaming-2"
          r.close
        end
      end
    ) do
      res = get_resp("/stream")
      refute res.key?("ETag")
    end
  end
end
