---
title: Request Logs
url: /middleware/log_requests
---

The request logging middleware allows you to define customized log statements to occur before and/or after each request is processed.

You can provide a log level and format string to be written before and after each request.


```ruby {filename=Itsi.rb}
log_requests \
  before: {
    level: "INFO",
    format: "[{request_id}] {method} {path_and_query} - {addr} "
  },
  after: {
    level: "INFO",
    format: "[{request_id}] └─ {status} in {response_time}"
  }
```


The log statement can populated with several different placeholders.
Available values are:

### `before` Format String
* `request_id` -  (A short, unique hexadecimal request identifier)
* `request_id_full` - (A full 128-bit unique request identifier)
* `method` - The HTTP method
* `path` - The HTTP Path
* `addr` - The client's IP address
* `host` - The request host
* `path_and_query` - The path and query combined
* `query` - The request query string
* `port` - The bound port
* `start_time` - The request start time

### `after` Format String
* `request_id` - (A short, unique hexadecimal request identifier)
* `request_id_full` - (A full 128-bit unique request identifier)
* `status` - The HTTP status code
* `addr` - The client's IP address
* `response_time` - The response time in milliseconds


### Path Attributes
In addition to this, any capture groups referenced by container location blocks
are also made available, to be interpolated into the log statement. E.g.:

```ruby {filename=Itsi.rb}

location "/users/:user_id" do # 1. If we capture user_id here.

  log_requests before: {
    level: "INFO",
    format: "[{request_id}] User: {user_id}" # 2. Then we can log the user_id here.
  }

end

```
