require_relative "../helpers/test_helper"
require 'debug'
class TestCompression < Minitest::Test

  def test_supports_compression
    require 'zlib'
    require 'stringio'

    payload = "I will be gzip compressed"
    server(
      itsi_rb: lambda do
        compress min_size: 0
        get("/foo"){|r| r.ok payload}
      end) do
        string_io = StringIO.new(get("/foo", {"Accept-Encoding" => "gzip"}))
        gzip_reader = Zlib::GzipReader.new(string_io)
        decompressed = gzip_reader.read
        gzip_reader.close
        assert_equal payload, decompressed
    end
  end

  def test_supports_compression_size_limit
    require 'zlib'
    require 'stringio'

    server(
      itsi_rb: lambda do
        compress min_size: 100
        get("/foo") do |r|
          r.ok "data" * r.query_params["repeat"].to_i
        end
      end) do

        string_io = StringIO.new(get("/foo?repeat=100", {"Accept-Encoding" => "gzip"}))
        gzip_reader = Zlib::GzipReader.new(string_io)
        decompressed = gzip_reader.read
        gzip_reader.close
        assert_equal "data" * 100, decompressed

        assert_equal get("/foo?repeat=1"), "data"
    end
  end

  def test_supports_compression_mime_type_conditions
    require 'zlib'
    require 'stringio'

    server(
      itsi_rb: lambda do
        compress mime_types: %w[image], min_size: 0
        get("/foo") do |r|
          r.respond("data", 200,  {"Content-Type" => r.query_params["type"]})
        end
      end) do

        string_io = StringIO.new(get("/foo?type=image/png", {"Accept-Encoding" => "gzip"}))
        gzip_reader = Zlib::GzipReader.new(string_io)
        decompressed = gzip_reader.read
        gzip_reader.close
        assert_equal "data", decompressed
        assert_equal get("/foo?type=application/octet-stream"), "data"
    end
  end

  def test_supports_compression_streaming_compression
    require 'zlib'
    require 'stringio'

    server(
      itsi_rb: lambda do
        compress mime_types: %w[all], min_size: 0, compress_streams: true
        get("/foo") do |req|
          r = req.response
          r << "one\n"
          r << "two\n"
          r << "three\n"
          r.close
        end
      end) do

        string_io = StringIO.new(get("/foo", {"Accept-Encoding" => "gzip"}))
        gzip_reader = Zlib::GzipReader.new(string_io)
        decompressed = gzip_reader.read
        gzip_reader.close
        assert_equal "one\ntwo\nthree\n", decompressed
    end
  end

end
