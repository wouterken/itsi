---
title: Listen Backlog
url: /options/listen_backlog
---

Configures the size of the listen backlog for the socket. Larger backlog sizes can improve performance for high-throughput applications by allowing more pending connections to queue, but may increase memory usage. The default value is 1024.

## Configuration
```ruby {filename=Itsi.rb}
listen_backlog 1024
```
