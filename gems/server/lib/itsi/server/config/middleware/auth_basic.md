---
title: Basic Authentication
url: /middleware/auth_basic
---
The Basic Auth middleware allows you to require Basic Authentication on any set of endpoints.

Valid credentials can be loaded from a credentials file (using Itsi’s built‑in [passfile generator](/utilities/passfile_generator)), or defined inline (for example via environment variables).

Keys are required to be hashed using one of the supported [hashing algorithms](/utilities/passfile_generator/#supported-hashing-algorithms).

## Configuration

### 1. Load from credentials file

```ruby {filename=Itsi.rb}
# Look for .itsi-credentials in the project root (format: key_id:secret per line)
auth_basic realm: "Admin Area", credentials_file: ".itsi-credentials"

# Default behavior. Looks for credentials file at .itsi-credentials
auth_basic

```

### 2. Inline credentials
```ruby {filename=Itsi.rb}
# Each key pair is identified by an ID
auth_basic realm: "Admin Area",  credentials_pairs: {
  "user_1" => ENV["BASIC_AUTH_PASSWORD_1"],
  "user_2" => ENV["BASIC_AUTH_PASSWORD_2"]
}
```

### 3. Apply Basic Authentication to specific endpoints

> See [location](/middleware/location)

```ruby {filename=Itsi.rb}
# Apply Basic Authentication to specific endpoints
location "/admin/*" do
  auth_basic realm: "Admin Area", credentials_pairs: {
    "user_1" => ENV["BASIC_AUTH_PASSWORD_1"],
    "user_2" => ENV["BASIC_AUTH_PASSWORD_2"]
  }
end
```
