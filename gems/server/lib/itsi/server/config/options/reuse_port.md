---
title: Reuse Port
url: /options/reuse_port
---

Configures whether the server should bind to the underlying socket using the `SO_REUSEPORT` option.
This option determines whether multiple sockets can listen on the same IP and port combination, which can improve load balancing and fault tolerance in multi-threaded or multi-process server applications.

The default value is `false`.

## Configuration
```ruby {filename=Itsi.rb}
reuse_port true
```

```ruby {filename=Itsi.rb}
reuse_port false
```
