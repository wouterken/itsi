---
title: Ruby Thread Request Backlog Size
url: /options/ruby_thread_request_backlog_size
---

Configures the size of the backlog queue for incoming requests in the Ruby thread pool.
Up to this many requests can be queued at once before the server rejects further requests to Ruby workers (note this does not block other requests to proxied hosts or for static assets).

The default value is `30 x number of threads`.

## Configuration
```ruby {filename=Itsi.rb}
ruby_thread_request_backlog_size 20
```

```ruby {filename=Itsi.rb}
ruby_thread_request_backlog_size 100
```
