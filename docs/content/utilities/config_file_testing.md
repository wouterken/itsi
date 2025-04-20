---
title: Config File Testing
type: docs
---
Itsi provides a convenience function to test the validity of an Itsi.rb config file
without having to start the server. (The server will automatically run this same process before
attempting to hot-reload config changes).

```bash
# Test config file at default location.
itsi test
```

```bash
# Test config file at custom path.
itsi test -C ./path/to/alternative.rb
```
