---
title: Cache-Control
url: /middleware/cache_control
---

The Cache-Control middleware allows you to configure HTTP caching headers for your application. It creates a standard `Cache-Control` header based on a set of directives and, optionally, an `Expires` header when a maximum age is specified. The middleware also supports setting a `Vary` header and any additional custom headers.

## Cache-Control configuration

```ruby
cache_control \
  max_age: 3600,
  s_max_age: 1800,
  stale_while_revalidate: 30,
  stale_if_error: 60,
  public: true,
  private: false,
  no_cache: false,
  no_store: false,
  must_revalidate: false,
  proxy_revalidate: false,
  immutable: false,
  vary: ["Accept-Encoding"],
  additional_headers: { "X-Custom-Header" => "HIT" }
```

## Cache-Control Applied to a sub-location

```ruby
location "/static" do
  cache_control \
    max_age: 86400,
    public: true,
    vary: ["Accept-Encoding", "User-Agent"]
  get("/assets") { |r| r.ok "static content" }
end
```

## Configuration Options

- **max_age**:
  An optional integer that sets the maximum time (in seconds) the response should be considered fresh. When specified, it also triggers the generation of an `Expires` header with the correct HTTP date.

- **s_max_age**:
  An optional integer for shared (proxy) cache time. It is set as `s-maxage=<value>` in the header.

- **stale_while_revalidate**:
  An optional integer that indicates how long (in seconds) a stale response may be served while revalidation occurs.

- **stale_if_error**:
  An optional integer that allows serving stale content if an error occurs during revalidation.

- **public**:
  A boolean flag. When `true` (and if `private` is not enabled), adds the `public` directive to the header.

- **private**:
  A boolean flag. When `true` (and if `public` is not enabled), adds the `private` directive to the header.

- **no_cache**:
  When `true`, the `no-cache` directive is added, instructing caches to validate the response with the origin server before reuse.

- **no_store**:
  When `true`, adds the `no-store` directive to completely disable caching.

- **must_revalidate**:
  When `true`, adds the `must-revalidate` directive ensuring stale responses are not used.

- **proxy_revalidate**:
  When `true`, the `proxy-revalidate` directive is added, which is similar to `must-revalidate` but for shared caches.

- **immutable**:
  When `true`, adds the `immutable` directive indicating that the response body will not change over time.

- **vary**:
  An array of header names as strings; these are concatenated and sent as the `Vary` header to inform caches which request headers might influence the response.

- **additional_headers**:
  A hash for any extra headers you wish to include in the response. Both keys and values are strings.
