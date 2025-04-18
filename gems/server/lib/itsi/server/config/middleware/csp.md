---
title: Content Security Policy
url: /middleware/csp
---

The `csp` middleware sets a [Content-Security-Policy (CSP)](https://developer.mozilla.org/en-US/docs/Web/HTTP/CSP) header on outgoing responses and optionally collects violation reports from clients.

### Options

| Key                 | Type        | Default         | Description |
|----------------------|-------------|------------------|-------------|
| `policy`      | `CspConfig` | `nil`            | Optional policy components for `default-src`, `script-src`, etc. |
| `reporting_enabled` | `Bool`      | `false`          | Enable collection of CSP violation reports |
| `report_file`       | `PathBuf`   | `nil`            | Where to persist reports if reporting is enabled |
| `report_endpoint`   | `String`    | `"/csp-report"`  | Endpoint to receive reports from the browser |
| `flush_interval`    | `Integer`   | `10`             | How frequently (in seconds) to flush pending reports to file |

### Example

```ruby
csp \
  policy: {
    default_src: ["'self'"],
    script_src: ["'self'", "cdn.example.com"],
    style_src: ["'self'"],
    report_uri: ["/csp-report"]
  },
  reporting_enabled: true,
  report_endpoint: "/csp-report",
  report_file: "csp_reports.json",
  flush_interval: 5
```

### Reporting
Configure `reporting_enabled`, `report_endpoint`, `report_file` and `flush_interval` to have Itsi perform CSP violation report collection.

If reporting is enabled, the middleware will collect violation reports from clients and persist them to the specified file at the given interval. (Make sure that `report_endpoint` and `report_uri` inside `policy_input` are correctly matched.)
