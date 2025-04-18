---
title: Log Format
url: /options/log_format
---

The `log_format` option toggles between JSON and plain text logging formats. By default, it is set to plain-text. For logging structured data into an external system, it is usually recommended to use the `JSON` format.

## Configuration
```ruby {filename=Itsi.rb}
log_format :plain
```

## Environment Variables
You can also set the `ITSI_LOG_FORMAT` environment variable to `json` or `plain` to control
this. If both are set, the configuration takes precedence.

## ANSI Escape codes.
By default Itsi tries to determine whether or not to use ANSI escape codes based on the output [target](/options/log_target). If you need to override this behavior, you can set the `ITSI_LOG_ANSI` environment variable to `true` or `false`.
