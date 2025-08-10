---
title: TLS Certificates
url: /options/certificates
---

Itsi can automatically generate TLS certificates for you, both in development and production environments. This includes support for Let's Encrypt ACME certificates with both HTTP-01 and TLS-ALPN-01 challenge types.

## Development / Self-signed
To automatically generate a TLS certificate in development, just bind using the `https` scheme.
E.g.
```ruby {filename=Itsi.rb}
bind "https://0.0.0.0"

# or

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

### Quick Setup
If you want to use Let's Encrypt to automatically generate a production TLS certificate, you can add `cert=acme` to the bind string.

E.g.
```ruby {filename=Itsi.rb}
bind "https://0.0.0.0?cert=acme&domains=example.com,example.org&acme_email=you@example.com"
```

### Advanced ACME Configuration
For more complex setups, use the `acme_certificates` configuration block:

```ruby {filename=Itsi.rb}
acme_certificates do
  # Global ACME settings
  contact_email "admin@example.com"
  cache_dir "/var/cache/itsi/acme"
  challenge_preference :http01  # or :tls_alpn01
  
  # Add certificates
  certificate ["example.com", "www.example.com"]
  certificate "api.example.com"
  
  # Certificate with custom settings
  certificate ["app.example.com", "cdn.example.com"] do
    auto_renew true
    auto_add true
  end
  
  # Event handling
  on_certificate_issued do |event|
    puts "Certificate issued for #{event[:domains].join(', ')}"
  end
  
  on_certificate_error do |event|
    puts "Certificate error: #{event[:error]}"
  end
end

# Bind to domains with automatic certificate resolution
bind "https://example.com"
bind "https://api.example.com"
```

## Challenge Types

Itsi supports both ACME challenge types:

### HTTP-01 Challenge (Recommended)
The HTTP-01 challenge works by serving a specific file at `http://yourdomain.com/.well-known/acme-challenge/` during certificate validation. This is the default and recommended method for most deployments.

```ruby {filename=Itsi.rb}
acme_certificates do
  contact_email "admin@example.com"
  challenge_preference :http01  # Default
  
  certificate ["example.com", "www.example.com"]
end
```

**Requirements:**
- Port 80 must be accessible from the internet
- Your server must be reachable at the domain being validated
- Works well with CDNs and reverse proxies that forward HTTP traffic

### TLS-ALPN-01 Challenge
The TLS-ALPN-01 challenge works by presenting a special certificate during the TLS handshake. Use this when HTTP-01 is not available.

```ruby {filename=Itsi.rb}
acme_certificates do
  contact_email "admin@example.com"
  challenge_preference :tls_alpn01
  
  certificate ["example.com", "www.example.com"]
end
```

**Requirements:**
- Port 443 must be accessible from the internet
- TLS termination must happen at your Itsi server (not at a CDN/proxy)
- Useful when port 80 is blocked or unavailable

## Runtime Certificate Management

Manage certificates programmatically during server operation:

```ruby {filename=Itsi.rb}
# In hooks or application code
after_start do
  # Add certificate dynamically
  Itsi::Server.add_certificate(["new.example.com"], "admin@example.com")
  
  # Check certificate status
  status = Itsi::Server.certificate_status(["example.com"])
  puts "Status: #{status['status']}"  # pending, active, expired, error, not_found
  
  # List all certificates
  certificates = Itsi::Server.list_certificates
  certificates.each do |cert|
    puts "#{cert['domains'].join(', ')}: #{cert['status']}"
  end
  
  # Manually renew certificate
  Itsi::Server.renew_certificate(["example.com"])
  
  # Remove certificate
  Itsi::Server.remove_certificate(["old.example.com"])
  
  # Manage challenge preferences
  Itsi::Server.set_challenge_preference(:http01)
  preference = Itsi::Server.get_challenge_preference
end
```

## Event Handling

Handle certificate lifecycle events:

```ruby {filename=Itsi.rb}
acme_certificates do
  contact_email "admin@example.com"
  
  # General event handler
  on_certificate_event do |event|
    case event[:type]
    when :issued
      puts "Certificate issued for #{event[:domains].join(', ')}"
      # Send notification, update monitoring, etc.
    when :renewed
      puts "Certificate renewed for #{event[:domains].join(', ')}"
      # Log renewal, update external systems, etc.
    when :error
      puts "Certificate error: #{event[:error]}"
      # Send alert, implement fallback, etc.
    end
  end
  
  # Specific event handlers
  on_certificate_issued do |event|
    # Handle successful certificate issuance
  end
  
  on_certificate_renewed do |event|
    # Handle successful certificate renewal
  end
  
  on_certificate_error do |event|
    # Handle certificate errors
  end
  
  certificate ["example.com", "www.example.com"]
end
```

## Environment Variables

ACME configuration can be controlled via environment variables:

- `ITSI_ACME_CONTACT_EMAIL`: Default contact email for ACME account registration
- `ITSI_ACME_CACHE_DIR`: Directory to cache certificates and account information
- `ITSI_ACME_DIRECTORY_URL`: ACME directory URL (overrides default Let's Encrypt)
- `ITSI_ACME_CA_PEM_PATH`: Optional CA PEM path for testing with custom CAs

```bash
export ITSI_ACME_CONTACT_EMAIL="admin@example.com"
export ITSI_ACME_CACHE_DIR="/var/cache/itsi/acme"
export ITSI_ACME_DIRECTORY_URL="https://acme-staging-v02.api.letsencrypt.org/directory"
```

Environment variables are overridden by explicit configuration in the `acme_certificates` block.

## Development and Testing

### Staging Environment
Test your ACME configuration with Let's Encrypt staging to avoid rate limits:

```ruby {filename=Itsi.rb}
acme_certificates do
  contact_email "admin@example.com"
  directory_url "https://acme-staging-v02.api.letsencrypt.org/directory"
  
  certificate ["staging.example.com"]
end
```

### Environment-specific Configuration
```ruby {filename=Itsi.rb}
if ENV["RAILS_ENV"] == "development"
  bind "https://localhost:8443"  # Self-signed certificate
elsif ENV["RAILS_ENV"] == "staging"
  acme_certificates do
    contact_email "admin@example.com"
    directory_url "https://acme-staging-v02.api.letsencrypt.org/directory"
    certificate ["staging.example.com"]
  end
  bind "https://staging.example.com"
else
  acme_certificates do
    contact_email "admin@example.com"
    cache_dir "/var/cache/itsi/acme"
    challenge_preference :http01
    
    certificate ["example.com", "www.example.com"]
    certificate "api.example.com"
  end
  bind "https://example.com"
  bind "https://api.example.com"
end
```

## Best Practices

{{< callout type="info" >}}
**Cache Directory**: Always configure a persistent cache directory in production to avoid re-requesting certificates on every server restart, which could hit Let's Encrypt rate limits.
{{< /callout >}}

{{< callout type="warn" >}}
**Rate Limits**: Let's Encrypt enforces strict rate limits (50 certificates per registered domain per week). Always test with the staging environment first.
{{< /callout >}}

{{< callout type="info" >}}
**Challenge Types**: HTTP-01 requires port 80 to be accessible for challenge validation. Use TLS-ALPN-01 if port 80 is blocked or behind a CDN that doesn't support HTTP-01 challenge forwarding.
{{< /callout >}}

{{< callout type="warn" >}}
**Domain Validation**: Ensure your server is accessible from the internet on the domains you're requesting certificates for. ACME validation requires external accessibility.
{{< /callout >}}

{{< callout type="info" >}}
**Automatic Renewal**: Certificates are automatically renewed when they're within 30 days of expiration. Use event handlers to get notified of renewal events.
{{< /callout >}}

## Troubleshooting

### Common Issues

**Port 80 blocked**: Use TLS-ALPN-01 challenge type
```ruby
challenge_preference :tls_alpn01
```

**Domain not accessible**: Ensure DNS points to your server and ports 80/443 are open

**Rate limits exceeded**: Use staging environment for testing
```ruby
directory_url "https://acme-staging-v02.api.letsencrypt.org/directory"
```

**Cache permissions**: Ensure cache directory is writable by the Itsi process
```ruby
cache_dir "/var/cache/itsi/acme"  # Ensure this is writable
```

### Certificate Status
Check certificate status programmatically:
```ruby
status = Itsi::Server.certificate_status(["example.com"])
case status["status"]
when "not_found"
  # Certificate hasn't been requested yet
when "pending"
  # Certificate request is in progress
when "processing"
  # ACME challenge is being processed
when "active"
  # Certificate is issued and active
when "expired"
  # Certificate has expired and needs renewal
when "error"
  # Certificate request failed - check logs
end
```
