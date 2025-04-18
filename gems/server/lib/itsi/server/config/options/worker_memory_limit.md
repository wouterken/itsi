---
title: Worker Memory Limit
url: /options/worker_memory_limit
---

The **Worker Memory Limit** option sets the maximum amount of memory a worker process can use before it is terminated. If an `after_memory_threshold_reached` hook is also set, you can define custom behaviour (e.g. an alert or exception notification) that should occur before the worker is rebooted.


## Configuration File

### Examples

```ruby {filename="Itsi.rb"}
worker_memory_limit 256 * 1024 ** 2 # 256 MB
```

```ruby {filename="Itsi.rb"}
worker_memory_limit 256 * 1024 ** 2 # 256 MB
after_memory_threshold_reached do |pid|
  send_slack_alert("Worker #{pid} exceeded memory limit")
end
```
