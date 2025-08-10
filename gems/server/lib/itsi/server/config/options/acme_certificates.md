---
title: ACME Certificate Management
url: /options/acme_certificates
---

Itsi provides comprehensive ACME (Automated Certificate Management Environment) support for automatic SSL/TLS certificate provisioning and management using Let's Encrypt and other ACME-compatible certificate authorities.

## Basic Configuration

Configure ACME certificates using the `acme_certificates` DSL block in your Itsi configuration:

```ruby {filename=Itsi.rb}
acme_certificates do
  contact_email "admin@example.com"
  cache_dir "/var/cache/itsi/acme"
  
  certificate ["example.com", "www.example.com"]
  certificate "api.example.com"
end
```

## Challenge Types

Itsi supports both HTTP-01 and TLS-ALPN-01 ACME challenge types:

```ruby {filename=Itsi.rb}
acme_certificates do
  # Set challenge preference (default: :http01)
  challenge_preference :http01  # or :tls_alpn01
  
  contact_email "admin@example.com"
  certificate ["example.com", "www.example.com"]
end
```

### HTTP-01 Challenge
The HTTP-01 challenge works by serving a specific file at `http://yourdomain.com/.well-known/acme-challenge/` during certificate validation. This is the recommended method for most deployments.

### TLS-ALPN-01 Challenge  
The TLS-ALPN-01 challenge works by presenting a special certificate during the TLS handshake. This method is useful when HTTP-01 is not available (e.g., port 80 is blocked).

## Configuration Options

### Global ACME Settings

```ruby {filename=Itsi.rb}
acme_certificates do
  # Contact email for ACME account registration
  contact_email "admin@example.com"
  
  # Directory to cache certificates (prevents re-issuance on restart)
  cache_dir "/var/cache/itsi/acme"
  
  # ACME directory URL (defaults to Let's Encrypt production)
  directory_url "https://acme-v02.api.letsencrypt.org/directory"
  
  # Challenge method preference
  challenge_preference :http01
end
```

### Individual Certificate Configuration

```ruby {filename=Itsi.rb}
acme_certificates do
  contact_email "admin@example.com"
  
  # Simple certificate
  certificate "example.com"
  
  # Multi-domain certificate
  certificate ["example.com", "www.example.com", "api.example.com"]
  
  # Certificate with custom email
  certificate "special.example.com", email: "special@example.com"
  
  # Certificate with advanced options
  certificate ["app.example.com", "cdn.example.com"] do
    auto_renew true    # Automatically renew before expiration (default: true)
    auto_add true      # Automatically request certificate on startup (default: true)
  end
end
```

## Event Handling

Handle certificate lifecycle events with custom callbacks:

```ruby {filename=Itsi.rb}
acme_certificates do
  contact_email "admin@example.com"
  
  # General event handler
  on_certificate_event do |event|
    puts "Certificate event: #{event[:type]} for #{event[:domains]}"
  end
  
  # Specific event handlers
  on_certificate_issued do |event|
    puts "Certificate issued for #{event[:domains].join(', ')}"
    # Send notification, update monitoring, etc.
  end
  
  on_certificate_renewed do |event|
    puts "Certificate renewed for #{event[:domains].join(', ')}"
    # Log renewal, update external systems, etc.
  end
  
  on_certificate_error do |event|
    puts "Certificate error for #{event[:domains].join(', ')}: #{event[:error]}"
    # Send alert, fallback to backup certificate, etc.
  end
  
  certificate ["example.com", "www.example.com"]
end
```

## Runtime Certificate Management

Manage certificates programmatically during server runtime:

```ruby {filename=Itsi.rb}
# In your application code or hooks
after_start do
  # Add certificate dynamically
  Itsi::Server.add_certificate(["new.example.com"], "admin@example.com")
  
  # Check certificate status
  status = Itsi::Server.certificate_status(["example.com"])
  puts "Certificate status: #{status['status']}"
  
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
  Itsi::Server.set_challenge_preference(:tls_alpn01)
  preference = Itsi::Server.get_challenge_preference
end
```

## Environment Variables

ACME configuration can also be controlled via environment variables:

- `ITSI_ACME_CONTACT_EMAIL`: Default contact email for ACME account registration
- `ITSI_ACME_CACHE_DIR`: Directory to cache certificates and account information
- `ITSI_ACME_DIRECTORY_URL`: ACME directory URL (overrides default Let's Encrypt)
- `ITSI_ACME_CA_PEM_PATH`: Optional CA PEM path for testing with custom CAs

```bash
export ITSI_ACME_CONTACT_EMAIL="admin@example.com"
export ITSI_ACME_CACHE_DIR="/var/cache/itsi/acme"
export ITSI_ACME_DIRECTORY_URL="https://acme-staging-v02.api.letsencrypt.org/directory"
```

## Integration with Bind Configuration

Combine ACME certificates with bind configurations for automatic HTTPS:

```ruby {filename=Itsi.rb}
# Configure ACME certificates
acme_certificates do
  contact_email "admin@example.com"
  challenge_preference :http01
  
  certificate ["example.com", "www.example.com"]
  certificate "api.example.com"
end

# Bind with automatic certificate resolution
bind "https://example.com"        # Uses ACME certificate for example.com
bind "https://api.example.com"    # Uses ACME certificate for api.example.com

# Or explicitly specify ACME
bind "https://example.com?cert=acme"
```

## Development and Staging

### Staging Environment

Test your ACME configuration with Let's Encrypt staging:

```ruby {filename=Itsi.rb}
acme_certificates do
  contact_email "admin@example.com"
  directory_url "https://acme-staging-v02.api.letsencrypt.org/directory"
  
  certificate ["staging.example.com"]
end
```

### Local Development

For local development, consider using self-signed certificates instead:

```ruby {filename=Itsi.rb}
if ENV["RAILS_ENV"] == "development"
  bind "https://localhost:8443"  # Auto-generates self-signed certificate
else
  acme_certificates do
    contact_email "admin@example.com"
    certificate ["example.com", "www.example.com"]
  end
  bind "https://example.com"
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

## Certificate Status

Certificate status can be one of:

- `not_found`: Certificate has not been requested
- `pending`: Certificate request is in progress
- `processing`: ACME challenge is being processed
- `active`: Certificate is issued and active
- `expired`: Certificate has expired
- `error`: Certificate request failed

## Error Handling

Handle certificate errors gracefully:

```ruby {filename=Itsi.rb}
acme_certificates do
  contact_email "admin@example.com"
  
  on_certificate_error do |event|
    case event[:error_type]
    when :rate_limit
      # Handle rate limiting
      puts "Rate limited for #{event[:domains].join(', ')}"
    when :validation_failed
      # Handle validation failures
      puts "Validation failed for #{event[:domains].join(', ')}: #{event[:error]}"
    when :network_error
      # Handle network issues
      puts "Network error for #{event[:domains].join(', ')}: #{event[:error]}"
    else
      puts "Unknown error for #{event[:domains].join(', ')}: #{event[:error]}"
    end
  end
  
  certificate ["example.com", "www.example.com"]
end
```
