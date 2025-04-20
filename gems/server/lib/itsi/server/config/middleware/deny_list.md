---
title: Deny List
url: /middleware/deny_list
next: endpoint/
---
The **deny_list** middleware restricts access to only those clients whose IP address matches one a set of approved patterns. All other requests receive a configurable forbidden response.

## Configuration

```ruby {filename=Itsi.rb}
deny_list \
  denied_patterns: [
    /192\.168\.0\.\d+/,     # block all 192.168.0.x
    /203\.0\.113\.(10|11)/, # block .10 and .11
    "10.0.0.0/24"           # block all IPs in the 10.0.0.x range
  ],
  error_response: { code: 403,
                    plaintext: { inline: "Access denied" },
                    default: "plaintext" }
```

*	`denied_patterns` (required):
An array of Ruby‑style regexp strings. Each incoming client IP (from req.addr) is tested against this set; if any match, the request is blocked.
*	`error_response` (optional):
A built‑in or custom error response (default is forbidden / HTTP 403).


## Trusted Proxies

By default, a deny-list uses the IP address from the underlying socket (remote_addr). However, if your server is behind a reverse proxy, all requests will appear to come from the proxy’s IP address. This can break IP-based rules or cause rate-limiting to group all users together.

To address this, you can declare trusted proxies and instruct the server to extract the original client IP from forwarded headers only if the request came from one of these proxies.

### Configuring trusted_proxies

To trust one or more upstream proxies, provide a trusted_proxies map in the middleware configuration.
E.g.
```ruby {filename=Itsi.rb}
deny_list \
  denied_patterns: ["10.0.0.0/8", /198\.51\.100\.\d+/],
  trusted_proxies: {
    "192.168.1.1" => { header: { name: "X-Forwarded-For" } }
  },
  error_response: { code: 403, plaintext: { inline: "Access denied" } }
```
