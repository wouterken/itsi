---
title: After Memory Limit Reached
url: /options/after_memory_limit_reached
---

The **after_memory_limit_reached** hook fires whenever a worker’s RSS memory usage exceeds a configured limit. It passes the PID of the process so you can log, alert, or take corrective action.

This option works in conjunction with the [worker_memory_limit](/options/worker_memory_limit).

```ruby {filename=Itsi.rb}
after_memory_limit_reached do |pid|
  AlertService.notify("Worker #{pid} memory exceeded limit")
end
```
