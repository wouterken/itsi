---
title: Static Response
url: /middleware/static_response
---

The **Static Response** middleware returns a fixed HTTP response immediately, without invoking any downstream handlers. You configure the status code, headers, and body content once, and every request is answered identically.

## Key Features
- **Fixed Status Code**: Return any valid HTTP status (200–599).
- **Custom Headers**: Pre‑set arbitrary headers (e.g. Content‑Type, Cache‑Control).
- **Arbitrary Body**: Supply text or binary data as the response payload.
- **Zero Routing**: Always handles the request; bypasses your application logic entirely.

## Example Usage
```ruby {filename=Itsi.rb}
static_response \
  code:    200,
  headers: [
    ["Content-Type", "application/json"],
    ["Cache-Control", "max-age=60"]
  ],
  body:    "{\"message\":\"OK\"}"
```

Every request now returns HTTP 200 with JSON body `{"message":"OK"}` and the prescribed headers.

## Configuration Options

| Option    | Type                        | Description                                                                                     |
|-----------|-----------------------------|-------------------------------------------------------------------------------------------------|
| **code**  | Integer                     | HTTP status code to return (e.g. 200, 404, 500).                                                |
| **headers**| Array of [String,String]   | List of header name/value pairs to include.                                                     |
| **body**  | Array<UInt8>                | Raw response body bytes. For text, use `string.bytes`.                                          |

```ruby
# Example in Itsi.rb
static_response \
  code:    404,
  headers: [
    ["Content-Type", "text/plain"],
    ["X-Error",        "NotFound"]
  ],
  body:    "Page not found"
```
