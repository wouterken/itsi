---
title: Error Response
url: /middleware/error_response
prev: schemas/
---

All of the below middlewares allow error responses to be customized (though will select a sane default if left unspecified).
* [`allow_list`](/middleware/allow_list)
* [`auth_api_key`](/middleware/auth_api_key)
* [`auth_jwt`](/middleware/auth_jwt)
* [`deny_list`](/middleware/deny_list)
* [`intrusion_protection`](/middleware/intrusion_protection)
* [`max_body`](/middleware/max_body)
* [`proxy`](/middleware/proxy)
* [`rate_limit`](/middleware/rate_limit)

You can override the default error responses by providing a custom error response, either selecting a different built-in type,
or providing a completely overridden options.
The following built-in types are available:
* `internal_server_error` (500)
* `not_found` (404)
* `unauthorized` (401)
* `forbidden` (403)
* `payload_too_large` (413)
* `too_many_requests` (429)
* `bad_gateway` (502)
* `service_unavailable` (503)
* `gateway_timeout` (504)

## Reuse a built-in type
To reuse a built-in type you can provide a string option for the `error_response` property.
E.g.
```ruby
auth_api_key .. other options.., error_response: 'forbidden'
```

## Example of built-in response
### HTML
  {{< card  title="Built-in error page" image="/error_page.jpg" subtitle="Default Itsi Error Page." method="Resize" options="10x q80 webp" >}}
### JSON
```json
{
  "error": "Too Many Requests",
  "message": "Too many requests within a limited time frame.",
  "code": 429,
  "status": "error"
}
```

## Override the error response
You may instead wish to completely override the error response. You can provide a status code, and a message in up to three
formats: plain-text, JSON, or HTML (at least one must be provided). Itsi will serve the appropriate type based on the `Accept` header of the incoming request, or fall back to the default if the requested type is not available.
Each of these three response formats can either be provided as an in-memory string, or a file path.

E.g.

```ruby
auth_api_key .. other options.., error_response: {
  status: 403,
  plaintext: nil, # No plain-text response provided
  json: { inline: {"message": "Forbidden"} }, # We provide the JSON response inline
  html: { file: './forbidden.html'}, # We provide the HTML response as a file path
  default: 'json' # When the Accept header doesn't match a supported response type, we'll default to JSON
}

# Or, e.g.
auth_api_key .. other options.., error_response: {
  status: 401,
  plaintext: "Unauthorized",
  json: { file: "unauthorized.json" },
  html: { inline: "<h1>Unauthorized</h1>" },
  default: 'html'
}
```
