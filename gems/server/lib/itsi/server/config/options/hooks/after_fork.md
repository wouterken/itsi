---
title: After Fork
url: /options/after_fork
parent: /options/after_fork
---

The **after_fork** hook runs once **in each worker process** immediately after it is forked. Use it to reinitialize connections (DB, cache) that shouldn't be shared across forks.

```ruby {filename=Itsi.rb}
after_fork do
  DB.reconnect!
end
```
