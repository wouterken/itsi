---
title: Deny List
url: /middleware/deny_list
---
The **deny_list** middleware restricts access to only those clients whose IP address matches one a set of approved patterns. All other requests receive a configurable forbidden response.

## Configuration

```ruby
deny_list \
  denied_patterns: [
    "^192\\.168\\.0\\.\\d+$",   # block all 192.168.0.x
    "^203\\.0\\.113\\.(10|11)$" # block .10 and .11
  ],
  error_response: { code: 403,
                    plaintext: { inline: "Access denied" },
                    default: "plaintext" }
```

*	`denied_patterns` (required):
An array of Ruby‑style regexp strings. Each incoming client IP (from req.addr) is tested against this set; if any match, the request is blocked.
*	`error_response` (optional):
A built‑in or custom error response (default is forbidden / HTTP 403).
