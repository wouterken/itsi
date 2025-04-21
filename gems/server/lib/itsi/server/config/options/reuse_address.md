---
title: Reuse Address
url: /options/reuse_address
---

Configures whether the server should bind to the underlying socket using the `SO_REUSEADDR` option.
This option determines whether the server allows the reuse of local addresses during binding. This can be useful in scenarios where a socket needs to be quickly rebound without waiting for the operating system to release the address.

## Configuration
```ruby {filename=Itsi.rb}
reuse_address true
```

```ruby {filename=Itsi.rb}
reuse_address false
```
