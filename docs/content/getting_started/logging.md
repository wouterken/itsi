---
title: Logging
type: docs
weight: 4
next: /configuration

---

## Targeted Logging
* Having trouble configuring a specific middleware layer, but debug logs are too verbose? You can change the log-level for a specific middleware layer,
while leaving all other layers at the current global level.
E.g.

```bash
# In this example, the auth_api_key middleware will log debug messages
# while everything else will stick to the INFO level.
ITSI_LOG=info,middleware::auth_api_key=debug itsi
```
