---
title: Logging
type: docs
weight: 4
next: /signals
---

Itsi has a very configurable logging system. You can configure logging use the `Itsi.rb` configuration file, environment variables or a combination of both.

## Basics
For basic logging needs, set a global log-level using the `ITSI_LOG` environment variable (to one of `trace`, `debug`, `info`, `warn`, `error`)

## Fine-grained control
Itsi supports fine-grained config-based control of logging behavior, which can be changed at runtime **without downtime**. To utilize these functions, read the documentation for the following options and middleware:

### Options
* [`log_level`](/options/log_level)
* [`log_target`](/options/log_target)
* [`log_format`](/options/log_format)
* [`log_target_filters`](/options/log_target_filters)
### Middleware
* [`log_requests`](/options/log_requests)
