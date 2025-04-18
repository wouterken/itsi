---
title: Stream Body
url: /options/stream_body
---

The **stream_body** option controls whether Itsi buffers the request body in memory before passing it to your application code. Several Rack applications rely on this behaviour (so that the request can be rewound and/or parsed in its entirety before being processed), for this reason the default behaviour is `true`.
However for light-weight requests that benefit from not holding the entire request body in memory, you can set this option to `false`.


{{< callout >}}
To prevent excessive memory usage, Itsi will store any large buffered request bodies in temporary storage on the file-system when request body streaming is disabled.
{{< /callout >}}



Streaming bodies cooperate particularly well with [Fiber scheduler](/options/fiber_scheduler) mode, to allow you to process many large and long-running incoming data streams simultaneously. For this use case, be sure to adjust both [max_body](/options/max_body) and [request_timeout](/options/request_timeout) to ensure large incoming requests are not terminated prematurely.



## Configuration File

```ruby
# Disable streaming the request body
stream_body false

# Enable streaming the request body
stream_body true
post("/test_stream") do |req|
  # req.body is an IO
  first_five_bytes = req.body.read(5)
end
```
