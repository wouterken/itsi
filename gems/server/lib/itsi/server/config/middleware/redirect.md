---
title: Redirect
url: /middleware/redirect
---

The Redirect middleware enables automatic redirection of incoming requests to a different URL. It computes the target URL using a string rewrite rule and responds with an HTTP redirect status code based on the configuration. Once the middleware processes a request, it immediately returns a redirection response without further processing.

## Redirect configuration

```ruby
redirect \
  to: "https://example.com/new-path", \
  type: "permanent"
```

## Redirect Applied to a sub-location

```ruby
location "/old-path" do
  redirect \
    to: "https://example.com/new-path", \
    type: "temporary"
  # No further route processing occurs for /old-path.
end
```
## Redirect HTTP to HTTPS

```ruby
location protocols: [:http] do
  redirect \
    to: "https://{host}{path_and_query}", \
    type: "moved_permanently"
end
```
> A shorthand for the above exists, simply call `redirect_http_to_https!`. Note that this is simply
an alias for the above, and as such is subject to ordinary [location](/middleware/location) resolution rules.
To make sure this rule takes precedence, place it above other locations in the `Itsi.rb` file.



## Configuration Options

- **to**:
  A [string rewrite rule](/middleware/string_rewrites) (or literal string) specifying the target URL for the redirection. This value can incorporate dynamic portions of the incoming request.

- **type**:
  Specifies the type of redirection. Allowed values include:
  - `"permanent"`: Responds with a 308 Permanent Redirect.
  - `"temporary"`: Responds with a 307 Temporary Redirect.
  - `"moved_permanently"`: Responds with a 301 Moved Permanently.
  - `"found"`: Responds with a 302 Found.

## How It Works

When a request is received, the Redirect middleware immediately intercepts it and builds a redirect response. The target URL is computed via the provided rewrite rule (`to`), and the response is sent with the appropriate HTTP status code as determined by the `type` option. No further handling is performed on the request.
