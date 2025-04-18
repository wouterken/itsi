---
title: Rate Limiter
url: /middleware/rate_limit
---

The **Rate Limiter** middleware enforces a fixed‑window rate limit on incoming requests. You configure a maximum number of requests allowed in a given time span, along with how to identify the client and where to store counters (in‑memory or Redis).

By default it limits per IP address using an in‑memory store, but you can:

- Change the window size (`requests` per `seconds`)
- Limit based on other attributes (header or query parameter)
- Swap to a Redis backend for cross‑process rate limiting
- Customize the error response when the limit is exceeded

## Configuration

```ruby
rate_limit \
  requests: 100,                   # max 100 requests
  seconds: 60,                     # per 60‑second window
  key: "address",                  # limit by client IP
  store_config: "in_memory",       # use local in‑memory store
  error_response: "too_many_requests"
```

### `requests` / `seconds`

- **`requests`**: Number of allowed requests in each window (positive integer).
- **`seconds`**: Length of each fixed window in seconds (positive integer).

### `key`

How to identify the client:

- **`"address"`** (default): use the client’s socket IP.
- **Header or query parameter**:
  ```ruby
  key: { parameter: { header: { name: "X-Api-Key-Id" } } }
  ```
  or
  ```ruby
  key: { parameter: { query:  { name: "user_id" } } }
  ```

### `store_config`

Where to keep counters:

- **`"in_memory"`** (default): per‑process, reset when server restarts.
- **Redis** (shared across workers):
  ```ruby
  store_config: { redis: { connection_url: "redis://localhost:6379/1" } }
  ```

### `error_response`

Customizes the response when the limit is reached (default is built‑in `too_many_requests`):

```ruby
error_response: {
  code: 429,
  plaintext: { inline: "Rate limit exceeded" },
  default:   "plaintext"
}
```

## Rate Limit Response Headers

When a request **exceeds** the allowed rate, the middleware returns your configured error response **plus** these headers:

| Header                    | Meaning                                                                                  |
|---------------------------|------------------------------------------------------------------------------------------|
| **X-RateLimit-Limit**       | The maximum number of requests allowed per window (`requests` value).                    |
| **X-RateLimit-Remaining**   | How many requests remain in the current window (will be `0` once the limit is hit).      |
| **X-RateLimit-Reset**       | Seconds until the current window resets (time until your counter zeroes out).            |
| **Retry-After**             | Same value as `X-RateLimit-Reset` — suggests when clients should retry their request.    |

> **Example**
> With `requests: 5, seconds: 60`, after 5 calls the 6th returns:
> ```
> 429 Too Many Requests
> X-RateLimit-Limit:     5
> X-RateLimit-Remaining: 0
> X-RateLimit-Reset:     42
> Retry-After:           42
> ```

## How It Works

1. **On each request**
   - Compute the client key (IP, header, or query).
   - Increment a counter for the current time window.

2. **Fixed‑window logic**
   - If the counter ≤ `requests`, allow through.
   - Otherwise, immediately return the configured `error_response` plus the **X-RateLimit** headers.

3. **Store options**
   - **In‑memory**: simple hash, fast but not shared across processes.
   - **Redis**: atomic `INCR` + `EXPIRE` commands, shared across all workers.

Place `rate_limit` anywhere in your routing DSL to apply it to all downstream handlers in that scope.
