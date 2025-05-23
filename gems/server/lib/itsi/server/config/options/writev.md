---
title: Write Vectored
url: /options/writev
---

Set whether HTTP/1 connections should try to use vectored writes,
or always flatten into a single buffer.

Note that setting this to false may mean more copies of body data,
but may also improve performance when an IO transport doesn't
support vectored writes well, such as most TLS implementations.

Setting this to true will force hyper to use queued strategy
which may eliminate unnecessary cloning on some TLS backends

Default is `nil` in which case hyper will try to guess which mode to use

## Configuration
```ruby {filename=Itsi.rb}
writev true
```

```ruby {filename=Itsi.rb}
writev false
```
