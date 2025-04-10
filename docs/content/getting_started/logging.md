---
title: Logging
type: docs
weight: 4
---

## Targeted Logging
* Having trouble configuring a specific middleware layer, but debug logs are too verbose? You can change the log-level for a specific middleware layer,
while leaving all other layers at the current global level.
E.g.

```bash
# auth_api_key middleware will log debug messages
# everything else will stick to the INFO level.
ITSI_LOG=info,auth_api_key=debug itsi
```
