---
title: API Key
url: /middleware/auth_api_key
---
The API key middleware allows you to protect any set of endpoints with an API Key requirement.

Valid API keys can be loaded from a credentials file (using Itsi’s built‑in [passfile generator](/utilities/passfile_generator)), or defined inline (for example via environment variables).

Keys are required to be hashed using one of the supported [hashing algorithms](/utilities/passfile_generator/#supported-hashing-algorithms).

{{< callout type="info" >}}
API keys may be **anonymous** (no ID; any valid secret will do), or **identified** (each secret is paired with a Key ID, and both must be supplied on each request).
{{< /callout >}}

## Configuration

### 1. Load from credentials file

```ruby {filename=Itsi.rb}
# Look for .itsi-credentials in the project root (format: key_id:secret per line)
auth_api_key credentials_file: ".itsi-credentials"

# Default behavior. Looks for credentials file at .itsi-credentials
auth_api_key

```

### 2. Inline anonymous keys

```ruby {filename=Itsi.rb}
# Only the secret values matter (no IDs)
auth_api_key valid_keys: [
  ENV["API_KEY_1"],
  ENV["API_KEY_2"]
]
```

### 3. Inline identified keys

```ruby {filename=Itsi.rb}
# Each key pair is identified by an ID
auth_api_key valid_keys: {
  "consumer_1" => ENV["API_KEY_1"],
  "consumer_2" => ENV["API_KEY_2"]
}
```

### 4. Apply API Key Auth to specific endpoints

> See [location](/middleware/location)

```ruby {filename=Itsi.rb}
# Apply Basic Authentication to specific endpoints
location "/admin/*" do
  auth_api_key valid_keys: {
    "consumer_1" => ENV["API_KEY_1"],
    "consumer_2" => ENV["API_KEY_2"]
  }
end
```


## Customized Key-ID and Secret sources
* The secret is expected inside an `Authorization` header, as a Bearer token.
* The Key-ID (*if not using anonymous auth*) is expected inside an `X-Api-Key-Id` header.
Both of these sources can be configured using the `key_id_source` and  `token_source` options.
The source can be either a named `header` (with optional prefix) or `query` parameter,
{{< callout >}}
Note: Using a query source for the *Secret* is not recommended, as full URLs are readily leaked and recorded via logs and browser history. You should reserve use of a query token-source for non-sensitive information or test cases.
{{< /callout >}}

```ruby {filename=Itsi.rb}
auth_api_key \
  valid_keys: {.. },
  key_id_source: { query: 'api_key_id' },
  token_source: { header: 'Authorization', prefix: 'Bearer ' }
```

## Customized Error Responses
This middleware will return a default `unauthorized` response if the API key is missing or invalid.
However you can override this behaviour, by providing a custom [error response](/middleware/error_response).
E.g.
```ruby {filename=Itsi.rb}
auth_api_key valid_keys: {.. }, error_response: "unauthenticated"
```

```ruby {filename=Itsi.rb}
auth_api_key valid_keys: {.. }, error_response: {code: 403, plaintext: {inline: "unauthenticated"} , default: 'plaintext'}

```
