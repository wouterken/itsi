---
title: Allow List
url: /middleware/allow_list
---
The **allow_list** middleware restricts access to only those clients whose IP address matches one of a set of approved patterns. All other requests receive a configurable forbidden response.

## Configuration

```ruby
allow_list \
  allowed_patterns: [
    /127\.0\.0\.1/,           # only localhost
    /10\.0\.\d+\.\d+/,        # any 10.0.x.x
    "192.168.1.0/24"          # CIDR range for 192.168.1.x
  ],
  error_response: "forbidden"
```

*	`allowed_patterns` (required):
An array of Ruby‑style regexp strings. Each incoming client IP (from req.addr) is tested against this set; if none match, the request is blocked.
*	`error_response` (optional):
A built‑in or custom error response (default is forbidden / HTTP 403).


## Trusted Proxies

By default, an allow-list uses the IP address from the underlying socket (remote_addr). However, if your server is behind a reverse proxy, all requests will appear to come from the proxy’s IP address. This can break IP-based rules or cause rate-limiting to group all users together.

To address this, you can declare trusted proxies and instruct the server to extract the original client IP from forwarded headers only if the request came from one of these proxies.


### Configuring trusted_proxies

To trust one or more upstream proxies, provide a trusted_proxies map in the middleware configuration.
E.g.
```ruby {filename=Itsi.rb}
allow_list \
  allowed_patterns: [
    /127\.0\.0\.1/,           # only localhost
    /10\.0\.\d+\.\d+/,        # any 10.0.x.x
    "192.168.1.0/24"          # CIDR range for 192.168.1.x
  ],
  trusted_proxies: {
    "192.168.1.1" => { header: { name: "X-Forwarded-For" } }
  }
```
