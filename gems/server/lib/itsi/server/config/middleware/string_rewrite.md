---
title: String Rewrites
url: /middleware/string_rewrites
weight: 5
---

The String Rewrite mechanism is used when configuring Itsi for
* [Reverse Proxying](/middleware/proxy)
* [Redirects](/middleware/redirect)
* [Logging](/middleware/log_requests)

It allows you to create dynamic strings from a template by combining literal text with placeholders. Placeholders (denoted using curly braces: `{}`) are replaced at runtime with data derived from the HTTP request, response, or context.

### Rewriting a Request

The following placeholders are supported:

- **`request_id`**: A short unique identifier for the request.
- **`request_id_full`**: The full request identifier.
- **`method`**: The HTTP request method (e.g., GET, POST).
- **`path`**: The URL path of the request.
- **`addr`**: The client IP address.
- **`host`**: The host portion of the URL (defaults to `localhost` if unspecified).
- **`path_and_query`**: The combination of the URL path and query string.
- **`query`**: The query string (prepended with a `?` if non-empty).
- **`port`**: The port number (defaulting to `80` if not available).
- **`start_time`**: The formatted start time of the request.

The mechanism also allows any available matching regex capture from routes defined in the [location](/middleware/location) block.
If no match is found, otherwise, the placeholder remains unchanged (i.e. it is rendered as `{placeholder_name}`).


## Example Templates

### Reverse Proxying

When acting as a reverse proxy, you might want to forward the request to a backend service. For example, if your backend service expects the complete path and query string appended to its URL, you could use:

```ruby
"https://backend.example.com/api{path}{query}"
```

For an incoming request to `/v1/resource?x=1`, this template rewrites the target URL to:
`https://backend.example.com/api/v1/resource?x=1`

### Redirects

For redirect middleware, a common use case is to guide clients from an old URL to a new one. For instance:

```ruby
"https://new.example.com{path}?source=redirect"
```

If a request comes in to `/old-section?foo=bar`, the rewrite produces:
`https://new.example.com/old-section?source=redirect`
