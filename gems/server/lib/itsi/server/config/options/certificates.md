---
title: TLS Certificates
url: /options/certificates
---

Itsi can automatically generate TLS certificates for you, both in development and production environments.

## Development / Self-signed
To automatically generate a TLS certificate in development, just bind using the `https` scheme.
E.g.
```ruby {filename=Itsi.rb}
bind "https://0.0.0.0"

or

bind "https://0.0.0.0:8443"
```
Itsi will generate a local development CA for you if it does not yet exist, then use this to
sign a just-in-time certificate for your server.
The local CA will by default live inside a `.itsi` directory inside your home directory.
This directory can be overwritten using the `ITSI_LOCAL_CA_DIR` environment variable.

You can add this CA to your system's trusted certificate store to avoid browser warnings in development.

If you want the generated certificate to be valid for specific domains, you can add these to your bind string, and they will be added as subject alternative names (SANs). For example:

```ruby {filename=Itsi.rb}
bind "https://example.com?domains=development.example.com,development.example.org"
```
## Existing Certificates
If you already have a certificate and key, you can use them by passing the path to the certificate and key files to the `bind` method.
E.g.
```ruby {filename=Itsi.rb}
bind "https://example.com?cert=/path/to/cert.pem&key=/path/to/key.pem"
```

## Production Certificates (Let's Encrypt)
If you want to use Let's Encrypt to automatically generate a production TLS certificate, you can add `cert=acme` to the bind string.

E.g.
```ruby {filename=Itsi.rb}
bind "https://0.0.0.0?cert=acme&domains=example.com,example.org&acme_email=you@example.com"
```

You can provide several ENV variables to configure further configure the Let's Encrypt integration:
- `ITSI_ACME_CACHE_DIR`: The directory to use to cache Let's encrypt certificates (so that these are not regenerated each time the server is restarted).
- `ITSI_ACME_CONTACT_EMAIL`: The email address to use for Let's Encrypt account registration (overridden by the `acme_email` parameter).
- `ITSI_ACME_CA_PEM_PATH`: Optional CA Pem path, used for testing with non-trusted CAs for certifcate generation (e.g. pebble)
- `ITSI_ACME_DIRECTORY_URL`: Override the ACME directory URL (e.g. https://acme-staging-v02.api.letsencrypt.org/directory).

{{< callout type="info" >}}
Let's Encrypt enforces strict rate limits on production certificate generation. To verify that your configuration is correct, it's recommended to test it first using the staging directory URL. E.g.
`ITSI_ACME_DIRECTORY_URL=https://acme-staging-v02.api.letsencrypt.org/directory`
{{< /callout >}}


Itsi supports both ACME challenge types that matter for common deployments:

* `TLS-ALPN-01` is used when the certificate authority can reach Itsi directly on the HTTPS listener.
* `HTTP-01` can be used when you also expose a reachable HTTP listener for the same hostname. In real Let's Encrypt deployments this typically means port `80` must reach Itsi for `/.well-known/acme-challenge/*`.

This means setups behind a CDN, WAF, or TLS-terminating proxy can still use automated certificates, provided plain HTTP validation traffic is forwarded to Itsi.

E.g. a production configuration that allows HTTP-01 fallback might look like this:

```ruby {filename=Itsi.rb}
bind "http://0.0.0.0:80"
bind "https://0.0.0.0:443?cert=acme&domains=example.com&acme_email=you@example.com"
```

## Dynamic Domain Registration
You can add or remove ACME-managed domains while Itsi is already running.

This is useful when hostnames are discovered dynamically by your Ruby application, or when you want to defer certificate issuance until a tenant, customer, or site is activated.

Runtime APIs:

* `Itsi::Server.tls_bindings`
* `Itsi::Server.tls_domains(listener_id = nil)`
* `Itsi::Server.tls_domain_statuses(listener_id = nil)`
* `Itsi::Server.register_tls_domain(domain, listener_id = nil)`
* `Itsi::Server.unregister_tls_domain(domain, listener_id = nil)`

Example:

```ruby
Itsi::Server.register_tls_domain("customer-a.example.com")

status = Itsi::Server.tls_domain_statuses.find { |entry| entry["domain"] == "customer-a.example.com" }
puts status
```

When using dynamic issuance with HTTP-01, the same requirement still applies: the domain being issued must be able to reach an Itsi-managed HTTP listener for the ACME challenge path.
