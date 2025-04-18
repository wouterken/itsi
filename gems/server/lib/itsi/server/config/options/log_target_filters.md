---
title: Log Target Filters
url: /options/log_target_filters
---
Log target filters allow you fine-grained control over which log messages are enabled based on their level and source. This is configured using the `EnvFilter` syntax, which supports filtering by log level, module path, or a combination of both. Examples of valid filters include:

- `"middleware=debug"`: Enables `debug` level logs for the `middleware` module and its submodules.
- `"middleware::auth_jwt=trace"`: Enables `trace` level logs for a specific submodule.
- `"debug,middleware::auth_jwt=warn,middleware::rate_limit=info"`: Combines multiple filters, setting different log levels for different modules.

These filters provide fine-grained control over logging behavior, making it easier to focus on relevant information during debugging or monitoring.
Configures the size of the listen backlog for the socket. Larger backlog sizes can improve performance for high-throughput applications by allowing more pending connections to queue, but may increase memory usage. The default value is 1024.

## Configuration
```ruby {filename=Itsi.rb}
log_target_filters ["middleware=debug", "middleware::rate_limit=trace"]
```
