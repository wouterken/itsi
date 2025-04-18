---
title: Reverse Proxy
url: /middleware/proxy
---

The Reverse Proxy middleware enables reverse proxying by forwarding incoming HTTP requests to one of several backend servers. It supports streaming requests and responses, uses a dynamic URL rewriting mechanism to compute the target URL, supports multiple backend selection strategies, and can override or add headers before forwarding requests.

## Proxy Configuration

```ruby
proxy \
  to: "http://backend.example.com/api{path}{query}", \
  backends: ["127.0.0.1:3001", "127.0.0.1:3002"], \
  backend_priority: "round_robin", \
  headers: { "X-Forwarded-For" => { rewrite: "{addr}" } }, \
  verify_ssl: false, \
  timeout: 30, \
  tls_sni: true, \
  error_response: "bad_gateway"
```
## Options
| **Option**            | **Description**                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
|-----------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `to`              | The target URL. It supports dynamic URL rewriting using placeholders like `{path}` and `{query}`. If no backends are given, it will perform DNS resolution to determine the appropriate backend to route to. This parameter also determines the SNI hostname and Host header (unless overridden).                                                                                                                              |
| `backends` (Optional) | An array of Socket addresses. E.g. `["127.0.0.1:3001", "127.0.0.1:3002"]`. If provided, all requests will be routed to one of these backends based on the selected `backend_priority` algorithm. **Note:** The proxy uses persistent connections, so repeated testing from the same client will typically be satisfied by the same backend.                                                           |
| `backend_priority` (Optional) | The strategy for selecting a backend server. Options include `round_robin`, `ordered`, and `random`. Defaults to `round_robin`.                                                                                                                                                                                                                                                                                                                        |
| `headers` (Optional)  | A hash of headers to be overridden or added before forwarding requests. To clear a header, set the value to `nil`.                                                                                                                                                                                                                                                                                                                                                 |
| `verify_ssl` (Optional) | A boolean indicating whether to verify SSL certificates. Defaults to `true`.                                                                                                                                                                                                                                                                                                                                                                                   |
| `timeout` (Optional)   | The timeout in seconds for the proxy request. Failures to respond in time will result in a 504 timeout error. Defaults to `30` seconds.                                                                                                                                                                                                                                                                                                                         |
| `tls_sni` (Optional)   | A boolean indicating whether to use TLS SNI. Defaults to `true`.                                                                                                                                                                                                                                                                                                                                                                                              |
| `error_response` (Optional) | The error response to be returned when the proxy fails. Users can either use a built-in error response or provide a custom one. See [Error Responses](/middleware/error_response) for more details on how to structure this. Defaults to the built-in `502 Bad Gateway` response.                                                                                                                |

## How It Works

1. **URL Rewriting**
   The `to` parameter is a dynamic template that applies the [String Rewrite](/middleware/string_rewrite) mechanism. For instance, placeholders such as `{path}` and `{query}` are replaced with parts from the incoming request.
   *Example*:
   - For a request to `/resource?id=5`, a template of
     ```ruby
     "http://backend.example.com/api{path}{query}"
     ```
     produces:
     ```
     http://backend.example.com/api/resource?id=5
     ```

2. **Backend Selection**
   The `backends` array lists available backend server addresses (formatted as `"IP:port"`). The `backend_priority` setting controls how a backend is chosen:
   - **round_robin**: Cycles sequentially through the list.
   - **ordered**: Always selects the first backend.
   - **random**: Chooses a backend at random.
   Note - The proxy uses persistent connections, so repeated testing from the same client will typically be satisfied by the same backend.

3. **Header Overrides**
   The `headers` option lets you specify extra or overriding headers. Each header value may be a literal or a string rewrite. For example, overriding `"X-Forwarded-For"` to carry the client’s IP is done by:
   ```ruby
   { "X-Forwarded-For" => { rewrite: "{addr}" } }
   ```

4. **Request Forwarding and Error Handling**
   Depending on whether the request method is idempotent, the middleware buffers the request body to allow retries, or streams it directly. If the target URL is invalid or a backend error occurs (e.g. timeout or connection error), a configurable error response is returned.
  See [Error Responses](/middleware/error_response) for more details on how to customize errors.
