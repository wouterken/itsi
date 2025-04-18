---
title: Shutdown Timeout
url: /options/shutdown_timeout
---

Sets the timeout for graceful shutdown of the server. Itsi will stop accepting new connections immediately after receiving a shutdown signal. Existing connections will be allowed to complete their requests within the specified timeout. Any connections that do not complete within the timeout will be forcefully closed.

### Default

```ruby {filename=Itsi.rb}
shutdown_timeout 5.0  # 5 seconds
```
### Example

```ruby {filename=Itsi.rb}
shutdown_timeout 20.0  # 20 seconds
```
