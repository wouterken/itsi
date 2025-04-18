---
title: Max Body
url: /options/max_body
---

Limits the maximum request body size in bytes. This helps prevent excessively large payloads, which can cause resource exhaustion or denial-of-service issues.

### Default

```ruby {filename=Itsi.rb}
max_body 10 * 1024 ** 2  # 10 MiB
```

### Example

```ruby {filename=Itsi.rb}
max_body 5 * 1024 ** 2  # 5 MiB
```
