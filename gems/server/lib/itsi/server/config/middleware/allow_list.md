---
title: Allow List
url: /middleware/allow_list
---
The **allow_list** middleware restricts access to only those clients whose IP address matches one a set of approved patterns. All other requests receive a configurable forbidden response.

## Configuration

```ruby
allow_list \
  allowed_patterns: [
    "^127\\.0\\.0\\.1$",       # only localhost
    "^10\\.0\\.\\d+\\.\\d+$"   # any 10.0.x.x
  ],
  error_response: "forbidden"
```

*	`allowed_patterns` (required):
An array of Ruby‑style regexp strings. Each incoming client IP (from req.addr) is tested against this set; if none match, the request is blocked.
*	`error_response` (optional):
A built‑in or custom error response (default is forbidden / HTTP 403).
