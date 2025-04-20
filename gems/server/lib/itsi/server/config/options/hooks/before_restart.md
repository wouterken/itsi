---
title: Before Restart
url: /options/before_restart
---

The **before_restart** hook runs once right before the server restarts after receiving a restart signal.

```ruby {filename=Itsi.rb}
before_restart do
  # this runs once before restarting
end
```
