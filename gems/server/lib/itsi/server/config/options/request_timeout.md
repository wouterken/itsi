---
title: Request Timeout
url: /options/request_timeout
---

Sets the maximum duration (in seconds) allowed for the entire request to complete. If exceeded, the request will be terminated.


### Default

```ruby {filename=Itsi.rb}
request_timeout 300  # 5 minutes
```
### Example

```ruby {filename=Itsi.rb}
request_timeout 60  # 1 minute
```


{{< callout type="warn" >}}
If the request is held-up inside a Ruby worker, the worker will be gracefully restarted (as killing in-progress threads is not safe).
{{< /callout >}}
