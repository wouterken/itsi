---
title: Header Read Timeout
url: /options/header_read_timeout
---

Sets the maximum time (in seconds) allowed to receive the request headers. This protects against slowloris-style attacks.

### Default

```ruby  {filename=Itsi.rb}
header_read_timeout 2.0
```
### Example

```ruby  {filename=Itsi.rb}
header_read_timeout 5.0
```
