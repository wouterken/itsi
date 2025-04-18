---
title: JWT Auth
url: /middleware/auth_jwt
---
The JWT authentication middleware allows you to require valid JWT Authentication for any set of endpoints.

Itsi supports verifying JWTs signed using each of the following algorithms: `HS256`, `HS384`, `HS512`, `RS256`, `RS384`, `RS512`, `ES256`, `ES384`, `ES512`, `PS256`, `PS384`, `PS512`.

## Configuration

### 1. Supporting multiple verifiers simultaneously
You can configure multiple verifiers for each algorithm, allowing you to rotate keys without downtime.

```ruby {filename=Itsi.rb}
auth_jwt verifiers: {
  "HS256" => [ENV['HS256_SECRET_1'], ENV['HS256_SECRET_2']],
  "RS512" => [ENV['RS512_SECRET_1'], ENV['RS512_SECRET_2']],
}
```

### 2. Further restrictions based on claims
You can further restrict access based on claims in the JWT payload. For example, you can require a specific role or scope. If claim restrictions are present and unmet, the request will be rejected.

```ruby {filename=Itsi.rb}
auth_jwt verifiers: {..},
  audiences: ["aud1", "aud2"],
  subjects: ["sub1", "sub2"],
  issuers: ["iss1", "iss2"]
```

### 3. Apply JWT Authentication to specific endpoints

> See [location](/middleware/location)

```ruby {filename=Itsi.rb}
# Apply Basic Authentication to specific endpoints
location "/admin/*" do
  auth_jwt verifiers: {..}
end
```

### 4. Leeway
You can optionally specify a leeway in seconds to account for clock skew between the client and server.

```ruby {filename=Itsi.rb}
auth_jwt verifiers: {..},
  leeway: 60
```

## Customized Token Source
* The JWT is expected inside an `Authorization` header, as a Bearer token.
This source can be overridden using the  `token_source` options.
A token source can be either a named `header` (with optional prefix) or `query` parameter,
{{< callout >}}
Note: Using a query source for the *Secret* is not recommended, as full URLs are readily leaked and recorded via logs and browser history. You should reserve use of a query token-source for non-sensitive information or test cases.
{{< /callout >}}

```ruby {filename=Itsi.rb}
auth_jwt \
  verifiers: {.. },
  token_source: { header: 'Authorization', prefix: 'Bearer ' }
```

## Verifier Secrets
* For `HMAC` algorithms, Itsi expects a `base64` encoded secret.
* For `RSA` (and `PS`) algorithms, Itsi expects a `PEM`-formatted key.
* For `ECDSA` algorithms, Itsi expects a `PEM`-formatted key.

Itsi's built-in [secrets management](/utilities/secrets_management) can be used to generate secrets for all supported algorithms.

## Customized Error Responses
This middleware will return a default `unauthorized` response if the API key is missing or invalid.
However you can override this behaviour, by providing a custom [error response](/middleware/error_response).
E.g.
```ruby {filename=Itsi.rb}
auth_jwt verifiers: {.. }, error_response: "unauthenticated"
```

```ruby {filename=Itsi.rb}
auth_jwt verifiers: {.. }, error_response: {code: 403, plaintext: {inline: "unauthenticated"} , default: 'plaintext'}

```
