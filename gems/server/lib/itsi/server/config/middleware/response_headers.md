---
title: Response Headers
url: /middleware/response_headers
---

The **Response Headers** middleware lets you add, override or remove HTTP headers **after** your application handler has produced a response. You can attach static values or dynamically compute them via [String Rewrite](/middleware/string_rewrite).

## Configuration
```ruby {filename=Itsi.rb}
response_headers \
  additions: {
    "X-Processed-By"        => ["Itsi"],
    "X-Response-Time-Millis" => ["{response_time}"]
  },
  removals: ["Server"]
```
- **additions** `Hash(String,Array(StringRewrite))`
  A map of header names to StringRewrite templates. Templates are evaluated against the final response and context in **after**.
- **removals** `Array(String)`
  A list of header names to delete before adding new ones.

## Examples
```ruby {filename=Itsi.rb}
# Stamp every response with our server name
response_headers \
  additions: { "X-Powered-By" => ["Itsi"] },
  removals:  []

# Remove the Server header and add timing info
response_headers \
  additions: { "X-Response-Time" => ["{response_time}"] },
  removals:  ["Server"]
```
