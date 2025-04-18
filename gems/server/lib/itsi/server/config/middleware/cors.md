---
title: CORS
url: /middleware/cors
---

The CORS middleware allows you to configure CORS settings for your application.
You can enable CORS for specific origins, methods, headers, and credentials.


## CORS configuration
```ruby {filename=Itsi.rb}
cors \
  allow_origins: ["*"],
  allow_methods: ["GET", "POST", "PUT", "DELETE"],
  allow_headers: ["Content-Type", "Authorization"],
  allow_credentials: true,
  expose_headers: ["X-Total-Count"],
  max_age: 3600
```



## CORS Applied to a sub-location
```ruby {filename=Itsi.rb}
location "/api" do
  cors \
    allow_origins: ["*"],
    allow_methods: ["GET", "POST", "PUT", "DELETE"],
    allow_headers: ["Content-Type", "Authorization"],
    allow_credentials: true,
    expose_headers: ["X-Total-Count"],
    max_age: 3600
end
```

## Configuration Options

You can customize the CORS behavior using the following options:

- **allow_origins**:
  A list of allowed origins (e.g., `"*"` or specific domain names).
  When credentials are allowed (see `allow_credentials`), the middleware echoes back the exact origin from the request.

- **allow_methods**:
  A list of allowed HTTP methods. Supported methods include:
  - `GET`
  - `POST`
  - `PUT`
  - `DELETE`
  - `OPTIONS`
  - `HEAD`
  - `PATCH`
  The internal implementation uses an enum (`HttpMethod`) with helper methods to match and convert these values.

- **allow_headers**:
  A list of headers that the client is allowed to include in its requests.

- **allow_credentials**:
  A boolean flag indicating whether credentials (like cookies or authorization headers) are allowed.

- **expose_headers**:
  A list of headers that browsers are allowed to access from the response.

- **max_age**:
  An optional field that sets the maximum time (in seconds) the result of a preflight request can be cached.

## How It Works

### Preflight Requests

For HTTP OPTIONS requests (used to determine if the actual request is safe to send):
#### 1.	Extraction of Request Headers
The middleware extracts the following from the incoming request:
*	`Origin`
*	`Access-Control-Request-Method`
*	`Access-Control-Request-Headers`

#### 2.	Validation via preflight_headers
These values are validated:
*	The Origin must be provided and permitted according to allow_origins.
*	The Access-Control-Request-Method must match one of the configured allow_methods.
*	Any headers listed in Access-Control-Request-Headers must appear in the allow_headers configuration.

#### 3.	Response Generation
If the validation succeeds, the middleware constructs a set of CORS headers including:
*	`Access-Control-Allow-Origin`
*	`Access-Control-Allow-Methods`
*	`Access-Control-Allow-Headers`
*	`Access-Control-Allow-Credentials` (if enabled)
*	`Access-Control-Max-Age` (if set)
*	`Access-Control-Expose-Headers` (if configured)

A response with status code 204 No Content is sent immediately, ending further processing.
