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


{{< callout type="warn" >}}
Currently only the TLS-ALPN-01 challenge type is supported for automated certificates.
The HTTP-01 challenge is not *yet* supported. This means that, for e.g. if your server is sitting behind a CDN or reverse proxy that performs HTTPS termination, you will not be able to rely on the *automated* certificate generation for fully automated, verified e2e encryption.

Instead you may wish to use:
* [Self-signed](#development--self-signed) certificates
* [Manually](#existing-certificates) install certificates
* Use HTTP between the CDN and the server
{{< /callout >}}
