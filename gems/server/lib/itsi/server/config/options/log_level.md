---
title: Log Level
url: /options/log_level
---

The `log_level` option allows you to control the verbosity of the logs. By default, it is set to `info`.
Valid values are `trace`, `debug`, `info`, `warn`, `error`, and `off`.

## Configuration
```ruby {filename=Itsi.rb}
log_level :debug
```

## Environment Variables
You can also set the `ITSI_LOG` environment variable to to control this.
If both are set, the configuration takes precedence.

### Syntax

**ITSI_LOG** uses [EnvFilter directive syntax](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#example-syntax).

E.g.
```bash
ITSI_LOG=warn # Set global log level
```


```bash
ITSI_LOG=info,middleware=debug # Set global log level, and override for all targets starting with middleware::*.
```

```bash
ITSI_LOG=warn,middleware::auth_jwt=debug,middleware=info #
```
