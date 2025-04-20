---
title: Before Shutdown
url: /options/before_shutdown
---

The **before_shutdown** hook runs once in the master process **before** the process shuts down.

```ruby {filename=Itsi.rb}
before_shutdown do
  # this runs in the master, once before shutdown
end
```
