---
title: Request Headers
url: /middleware/request_headers
---

The **Request Headers** middleware lets you add, override or remove HTTP headers **before** your application handler sees the request. You can inject static values or use the full power of [String Rewrite](/middleware/string_rewrite) to generate header values based on path, query, method, etc.

## Configuration
```ruby {filename=Itsi.rb}
request_headers \
  additions: {
    "X-Correlation-ID" => ["{request_id_full}"],
    "X-Env"            => ["production"]
  },
  removals: ["X-Forwarded-For"]
```

- **additions** (`Hash(String,Array(StringRewrite))`)
  A map of header names to an array of StringRewrite templates. Each template is rendered **after** removals.
- **removals** `Array(String)`
  A list of header names to delete outright. If you want to override an existing header, include it in **both** removals and additions.

## Examples
```ruby {filename=Itsi.rb}
# Add a per‑request UUID and drop any X‑Forwarded‑For header
request_headers \
  additions: { "X-Correlation-ID" => ["{request_id}"] },
  removals:  ["X-Forwarded-For"]

# Inject the full request path and query
request_headers \
  additions: { "X-Full-URL" => ["{path_and_query}"] },
  removals:  []
```
