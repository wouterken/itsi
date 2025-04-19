---
title: After Start
url: /hooks/after_start
---

The **after_start** hook runs once **in each worker process and the master process** after the server start-up process has completed. (Note that operations performed in this hook should be idempotent, as this hook will be executed multiple times in cluster mode.)

```ruby {filename=Itsi.rb}
after_fork do
  DB.reconnect!
end
```
