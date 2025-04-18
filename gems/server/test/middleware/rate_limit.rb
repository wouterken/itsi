require_relative "../helpers/test_helper"
require "redis"

class TestRateLimit < Minitest::Test

  # 1. In‑memory: allow up to N requests, then 429
  def test_in_memory_limit
    server(
      itsi_rb: lambda do
        rate_limit requests: 3, seconds: 2
        get("/foo") { |r| r.ok "ok" }
      end
    ) do
      3.times do
        res = get_resp("/foo")
        assert_equal "200", res.code
        assert_equal "ok",  res.body
      end
      # Next one should be rate‑limited
      res = get_resp("/foo")
      assert_equal "429", res.code
      # default error_response body is the standard message
      assert_match /Slow down!/, res.body
      assert_match /429/, res.body

      assert_equal 3.to_s,   res["X-RateLimit-Limit"]
      assert_equal "0",                     res["X-RateLimit-Remaining"]
      assert_match  /\d+/,                  res["X-RateLimit-Reset"]
      assert_equal res["X-RateLimit-Reset"], res["Retry-After"]
    end
  end

  # 2. Time window resets after `seconds`
  def test_window_reset
    server(
      itsi_rb: lambda do
        rate_limit requests: 1, seconds: 1
        get("/bar") { |r| r.ok "bar" }
      end
    ) do
      res1 = get_resp("/bar")
      assert_equal "200", res1.code

      res2 = get_resp("/bar")
      assert_equal "429", res2.code

      sleep 1.1
      res3 = get_resp("/bar")
      assert_equal "200", res3.code
    end
  end

  # 3. Key by header
  def test_key_by_header
    server(
      itsi_rb: lambda do
        rate_limit \
          requests: 1,
          seconds: 60,
          key: { parameter: { header: { name: "X-Client-Id" } } }
        get("/h") { |r| r.ok r.header("X-Client-Id").first }
      end
    ) do
      h1 = { "X-Client-Id" => "A" }
      h2 = { "X-Client-Id" => "B" }

      # A once OK, then limited
      res1 = get_resp("/h", h1)
      assert_equal "200", res1.code
      assert_equal "A",   res1.body

      res2 = get_resp("/h", h1)
      assert_equal "429", res2.code

      # B independent count
      res3 = get_resp("/h", h2)
      assert_equal "200", res3.code
      assert_equal "B",   res3.body
    end
  end

  # 4. Key by query
  def test_key_by_query
    server(
      itsi_rb: lambda do
        rate_limit \
          requests: 1,
          seconds: 60,
          key: { parameter: { query: "user" } }
        get("/q") { |r| r.ok r.query_params["user"] }
      end
    ) do
      res1 = get_resp("/q?user=foo")
      assert_equal "200", res1.code
      assert_equal "foo", res1.body

      res2 = get_resp("/q?user=foo")
      assert_equal "429", res2.code
    end
  end

  # 5. Custom error_response
  def test_custom_error_response
    server(
      itsi_rb: lambda do
        rate_limit \
          requests: 1,
          seconds: 60,
          error_response: {
            code: 429,
            plaintext: { inline: "Slow down" },
            default:   "plaintext"
          }
        get("/") { |r| r.ok "never" }
      end
    ) do
      res = 5.times.map{ get_resp("/") }.last
      assert_equal "429", res.code
      assert_equal "Slow down", res.body
    end
  end

  # 6. Skip Redis tests if Redis not available
  def test_redis_store_unavailable_skips
    skip "Redis not running" unless begin
      Redis.new(url: ENV.fetch("REDIS_URL","redis://localhost:6379/15")).ping == "PONG"
    rescue
      false
    end
  end

  # 7. Redis‑backed limiting
  def test_redis_backed_limit
    redis_url = ENV.fetch("REDIS_URL","redis://localhost:6379/15")
    ENV["REDIS_URL"] = redis_url
    server(
      itsi_rb: lambda do
        rate_limit \
          requests: 2,
          seconds: 60,
          store_config: { redis: { connection_url: ENV["REDIS_URL"] } }
        get("/r") { |r| r.ok "ok" }
      end
    ) do
      client = Redis.new(url: redis_url)
      client.flushdb

      2.times do
        res = get_resp("/r")
        assert_equal "200", res.code
        assert_equal "ok",  res.body
      end

      # Third hit is rate‑limited
      res3 = get_resp("/r")
      assert_equal "429", res3.code

      client.flushdb
    end
  end
end
