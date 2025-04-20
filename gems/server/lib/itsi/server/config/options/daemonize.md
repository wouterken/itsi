---
title: Daemonize
url: /options/daemonize
---

If you set `daemonize` to true, the server will run in the background.
(Setting `daemonize` back to false, while auto-reloading config will not return the service to the foreground).

## Configuration
```ruby {filename=Itsi.rb}
daemonize true
```

Use `status`, `start` and `stop` to inspect server state.
