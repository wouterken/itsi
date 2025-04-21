---
title: Nodelay
url: /options/nodelay
---

Configures whether the server should enable the `TCP_NODELAY` option on the underlying socket.
This option determines whether the Nagle's algorithm is disabled, allowing small packets of data to be sent immediately without waiting for more data to fill the packet.

## Configuration
```ruby {filename=Itsi.rb}
nodelay true
```

```ruby {filename=Itsi.rb}
nodelay false
```
