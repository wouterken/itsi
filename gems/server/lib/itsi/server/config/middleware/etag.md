---
title: ETag
url: /middleware/etag
---

The ETag middleware enables HTTP entity tag generation for responses. It provides cache validation by automatically computing and attaching an ETag to outgoing responses, and optionally responding with `304 Not Modified` if the client includes a matching `If-None-Match` header.

ETags are useful for optimizing client-side caching, conditional GETs, and reducing unnecessary data transfer.


## ETag configuration
```ruby {filename=Itsi.rb}
etag \
  type: "strong",
  algorithm: "sha256",
  min_body_size: 0
```

## ETag Applied to a sub-location
```ruby {filename=Itsi.rb}
location "/assets" do
  etag \
    type: "weak",
    algorithm: "md5",
    min_body_size: 1024
end
```

## Configuration Options

- **type**: Specifies whether the generated ETag is `"strong"` or `"weak"`.
  - `strong`: ETag changes with any byte-level difference in the body.
  - `weak`: Indicates a semantic equivalence rather than byte-level identity.

- **algorithm**: Specifies the hash algorithm used to compute the ETag.
  - `sha256` (default)
  - `md5`

- **min_body_size**: Minimum response body size (in bytes) required before an ETag is generated. Use this to skip ETags for small or trivial responses.

## How It Works

### Before the Response
If  the request includes an `If-None-Match` header, the value is stored in the request context for comparison later.

### After the Response

1. If the status code is cacheable (e.g., 200 OK, 201 Created), the middleware proceeds.
2. If an ETag header is already present or if `Cache-Control: no-store` is set, it skips computation.
3. If the response body is not streamable and meets the `min_body_size`, it is buffered.
4. A hash (SHA-256 or MD5) is computed from the full body content.
5. The ETag header is inserted using either strong (`"abc123"`) or weak (`W/"abc123"`) formatting.
6. If the incoming request had a matching `If-None-Match`, the response is replaced with a 304 Not Modified.

---
