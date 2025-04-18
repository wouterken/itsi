---
title: Intrusion Protection
url: /middleware/intrusion_protection
---

The **Intrusion Protection** middleware detects and automatically bans clients that attempt to access suspicious URLs or send malicious header values. It combines pattern‑based detection (on request paths and header values) with a back‑end ban manager to temporarily block offending IPs.

- **URL Patterns**: a list of regexes; any matching request path causes an immediate ban.
- **Header Patterns**: per‑header regex lists; any matching header value causes a ban.
- **Ban Duration**: how long (in seconds) to block the client IP.
- **Store**: in‑memory or Redis‑backed (`store_config`) for both tracking and bans.
- **Error Response**: customizable (default is `forbidden`).

## Configuration

```ruby {filename=Itsi.rb}
intrusion_protection \
  banned_url_patterns: [
    "/admin/login",         # brute‑force login attempts
    /\.php$/               # any PHP‑extension request
  ],
  banned_header_patterns: {
    "User-Agent" => [
      "sqlmap",             # SQL injection scanner
      "curl"                # script‑based probing
    ]
  },
  banned_time_seconds: 300, # ban for 5 minutes
  store_config: "in_memory",# or { redis: { connection_url: "redis://…" } }
  error_response: "forbidden"
```

### Using Known‑Paths Helpers

Itsi provides a `KnownPaths` module with many pre‑assembled lists of common attack targets taken from [FuzzDB](https://blog.mozilla.org/security/2013/08/16/introducing-fuzzdb/) (e.g. typical login or backup file locations). Each helper returns an `Array<String>` you can pass directly:

```ruby {filename=Itsi.rb}
# ban all common WordPress plugin endpoints
intrusion_protection \
  banned_url_patterns: Itsi::Server::KnownPaths.cms_wp_plugins,
  banned_time_seconds: 600

# ban both login files and directory‑brute paths
intrusion_protection \
  banned_url_patterns: (
    Itsi::Server::KnownPaths.login_file_locations_logins +
    Itsi::Server::KnownPaths.filename_dirname_bruteforce_common_web_extensions
  ).uniq,
  banned_time_seconds: 900
```

Available helper methods live under `Itsi::Server::KnownPaths`—for example:

- `login_file_locations_logins`
- `filename_dirname_bruteforce_test_demo`
- `cms_wp_plugins`
- `php_php_common_backdoors`
- …and many more.
To see all options, execute
```ruby
Itsi::Server::KnownPaths::ALL
```
in a REPL or see the raw input files [here](https://github.com/wouterken/itsi/tree/main/gems/server/lib/itsi/server/config/known_paths).

### Options

- **banned_url_patterns** (Array<String>)
  Regexes applied to the full `path_and_query` of each request. A match → immediate ban+403.
- **banned_header_patterns** (Hash<String,Array<String>>)
  For each header name, a list of regexes tested against that header’s value. A match → ban+403.
- **banned_time_seconds** (Integer)
  Duration (in seconds) to keep the client IP banned.
- **store_config** (`"in_memory"` or `{ redis: { connection_url: String } }`)
  Backend for counters and ban state.
- **error_response** (String or detailed ErrorResponse)
  Response returned on detection or if IP is already banned (default: `forbidden`).

## How It Works

1. **Initialization**
   - Compile `banned_url_patterns` into a `RegexSet`.
   - Compile each set of header patterns into its own `RegexSet`.
   - Instantiate a `RateLimiter` and `BanManager` (in‑memory or Redis).

2. **Per‑Request**
   - **Check ban status**: if the IP is already banned, return `error_response` immediately.
   - **URL check**: if the request’s `path_and_query` matches any banned URL pattern, ban the IP for `banned_time_seconds` and return `error_response`.
   - **Header check**: for each configured header, if its value matches any banned pattern, ban the IP and return `error_response`.
   - Otherwise, allow the request to proceed.

Banned IPs are automatically un‑banned after the specified TTL.
