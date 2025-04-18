---
title: Secrets Management
type: docs
---

You can use Itsi to generate secrets for use within [JWT](/middleware/auth_jwt) middleware.

### Save new secret to secrets directory
```bash
itsi secret -d ./secrets
```

### Print new secret to STDOUT
```bash
itsi secret
```

### Support Secret Algorithms:
* `HS256`
* `HS384`
* `HS512`
* `RS256`
* `RS384`
* `RS512`
* `PS256`
* `PS384`
* `PS512`
* `ES256`
* `ES384`
