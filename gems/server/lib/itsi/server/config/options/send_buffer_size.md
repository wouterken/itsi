---
title: Send Buffer Size
url: /options/send_buffer_size
---

Configures the size of the send buffer for the socket. Larger buffer sizes can improve performance for high-throughput applications but may increase memory usage. The default value is 262,144 bytes.

## Configuration
```ruby {filename=Itsi.rb}
send_buffer_size 262_144
```

```ruby {filename=Itsi.rb}
send_buffer_size 1_048_576
```
