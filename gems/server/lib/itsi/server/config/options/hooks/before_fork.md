---
title: Before Fork
url: /options/before_fork
---

The **before_fork** hook runs once in the master process **before** any worker processes are spawned. Use it to preload resources, open shared connections, or otherwise prepare global state.

```ruby {filename=Itsi.rb}
before_fork do
  # this runs in the master, once before forking
  MyCache.connect!(url: ENV["CACHE_URL"])
end
```
