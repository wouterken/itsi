---
title: Passfile Generator
type: docs
---
Itsi comes bundled with a passfile generator for managing passfiles
containing hashed passwords for use within the [Basic Auth](/middleware/auth_basic) and [API Key](/middleware/auth_api_key) middleware.

## Maintaining a passfile
Use the `itsi passfile` subcommand to manage your passfiles.

### Adding/overwriting an entry
```bash
itsi passfile add --passfile=<path_to_passfile>
```

### Generate/echo an entry (without saving it)
```bash
itsi passfile echo
```

### Change hash function
```bash
itsi passfile add --passfile=<path_to_passfile> --algorithm=bcrypt
itsi passfile echo --algorithm=argon2
```

### Removing an entry
```bash
itsi passfile remove --passfile=<path_to_passfile>
```

### Listing all entries
```bash
itsi passfile list --passfile=<path_to_passfile>
```
### Supported Hashing Algorithms
* `argon2`
* `bcrypt`
* `sha256`
* `sha512`
* `none`
