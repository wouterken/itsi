---
title: ACME Certificate Management
type: docs
weight: 5
---

# ACME Certificate Management

Itsi provides comprehensive support for automated SSL/TLS certificate management using ACME (Automated Certificate Management Environment) protocol, including Let's Encrypt and other ACME-compatible certificate authorities.

## Overview

Itsi's ACME implementation supports:

- **Multiple Challenge Types**: HTTP-01 and TLS-ALPN-01 challenges
- **Automatic Certificate Provisioning**: Request certificates automatically on server startup
- **Certificate Lifecycle Management**: Automatic renewal, expiration tracking, and event handling
- **Runtime Management**: Add, remove, and renew certificates during server operation
- **Persistent Caching**: Store certificates and account information to avoid re-issuance
- **Event-Driven Architecture**: Handle certificate events with custom callbacks

## Quick Start

### Simple Configuration

For basic setups, configure ACME certificates using the bind parameter:

```ruby
# Itsi.rb
bind "https://0.0.0.0?cert=acme&domains=example.com,www.example.com&acme_email=admin@example.com"
```

### Advanced Configuration

For production deployments, use the `acme_certificates` configuration block:

```ruby
# Itsi.rb
acme_certificates do
  contact_email "admin@example.com"
  cache_dir "/var/cache/itsi/acme"
  challenge_preference :http01
  
  certificate ["example.com", "www.example.com"]
  certificate "api.example.com"
  
  on_certificate_issued do |event|
    puts "Certificate issued for #{event[:domains].join(', ')}"
  end
end

bind "https://example.com"
bind "https://api.example.com"
```

## Challenge Types

### HTTP-01 Challenge (Recommended)

The HTTP-01 challenge validates domain ownership by serving a specific file at `http://yourdomain.com/.well-known/acme-challenge/` during certificate validation.

```ruby
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

**Best for:**
- Standard web deployments
- Servers behind CDNs or load balancers
- Multi-domain certificates

### TLS-ALPN-01 Challenge

The TLS-ALPN-01 challenge validates domain ownership through a special certificate presented during the TLS handshake.

```ruby
acme_certificates do
  contact_email "admin@example.com"
  challenge_preference :tls_alpn01
  
  certificate ["example.com", "www.example.com"]
end
```

**Requirements:**
- Port 443 must be accessible from the internet
- TLS termination must happen at your Itsi server
- Cannot be used with CDNs that terminate TLS

**Best for:**
- Direct server deployments
- When port 80 is blocked or unavailable
- Internal services with restricted network access

## Configuration Options

### Global Settings

```ruby
acme_certificates do
  # Required: Contact email for ACME account registration
  contact_email "admin@example.com"
  
  # Optional: Directory to cache certificates and account data
  cache_dir "/var/cache/itsi/acme"
  
  # Optional: ACME directory URL (defaults to Let's Encrypt production)
  directory_url "https://acme-v02.api.letsencrypt.org/directory"
  
  # Optional: Challenge method preference (default: :http01)
  challenge_preference :http01  # or :tls_alpn01
end
```

### Certificate Configuration

```ruby
acme_certificates do
  contact_email "admin@example.com"
  
  # Single domain certificate
  certificate "example.com"
  
  # Multi-domain certificate (SAN)
  certificate ["example.com", "www.example.com", "api.example.com"]
  
  # Certificate with custom email
  certificate "special.example.com", email: "special@example.com"
  
  # Certificate with advanced options
  certificate ["app.example.com", "cdn.example.com"] do
    auto_renew true    # Automatically renew before expiration (default: true)
    auto_add true      # Request certificate on startup (default: true)
  end
end
```

## Event Handling

Handle certificate lifecycle events with custom callbacks:

### General Event Handler

```ruby
acme_certificates do
  contact_email "admin@example.com"
  
  on_certificate_event do |event|
    case event[:type]
    when :issued
      puts "✅ Certificate issued for #{event[:domains].join(', ')}"
      send_notification("Certificate issued", event)
    when :renewed
      puts "🔄 Certificate renewed for #{event[:domains].join(', ')}"
      update_monitoring(event)
    when :error
      puts "❌ Certificate error: #{event[:error]}"
      send_alert("Certificate error", event)
    end
  end
  
  certificate ["example.com", "www.example.com"]
end
```

### Specific Event Handlers

```ruby
acme_certificates do
  contact_email "admin@example.com"
  
  # Certificate successfully issued
  on_certificate_issued do |event|
    # Update external systems
    update_load_balancer_certificates(event[:domains])
    update_cdn_certificates(event[:domains])
    
    # Send notification
    slack_notify("Certificate issued for #{event[:domains].join(', ')}")
  end
  
  # Certificate successfully renewed
  on_certificate_renewed do |event|
    logger.info "Certificate renewed for #{event[:domains].join(', ')}"
    update_metrics("certificate_renewed", event[:domains])
  end
  
  # Certificate operation failed
  on_certificate_error do |event|
    case event[:error_type]
    when :rate_limit
      schedule_retry(event[:domains], delay: 1.hour)
    when :validation_failed
      verify_dns_configuration(event[:domains])
    else
      send_alert("Certificate error: #{event[:error]}")
    end
  end
  
  certificate ["example.com", "www.example.com"]
end
```

## Runtime Management

Manage certificates programmatically during server operation:

### Adding Certificates

```ruby
# In your application or hooks
after_start do
  # Add a new certificate
  Itsi::Server.add_certificate(["new.example.com"], "admin@example.com")
  
  # Add multi-domain certificate
  Itsi::Server.add_certificate(
    ["app.example.com", "www.app.example.com"], 
    "app-admin@example.com"
  )
end
```

### Certificate Status

```ruby
# Check certificate status
status = Itsi::Server.certificate_status(["example.com"])

case status["status"]
when "not_found"
  # Certificate hasn't been requested
when "pending"
  # Certificate request in progress
when "processing"
  # ACME challenge being processed
when "active"
  # Certificate issued and active
when "expired"
  # Certificate expired, needs renewal
when "error"
  # Certificate request failed
end

# Status includes additional information
puts "Domains: #{status['domains']}"
puts "Email: #{status['acme_email']}"
puts "Expires: #{status['expires_at']}"
```

### Certificate Management

```ruby
# List all certificates
certificates = Itsi::Server.list_certificates
certificates.each do |cert|
  puts "#{cert['domains'].join(', ')}: #{cert['status']}"
  puts "  Email: #{cert['acme_email']}"
  puts "  Created: #{cert['created_at']}"
  puts "  Expires: #{cert['expires_at']}"
end

# Manually renew certificate
Itsi::Server.renew_certificate(["example.com"])

# Remove certificate
Itsi::Server.remove_certificate(["old.example.com"])
```

### Challenge Preferences

```ruby
# Get current challenge preference
preference = Itsi::Server.get_challenge_preference
puts "Current preference: #{preference}"  # :http01 or :tls_alpn01

# Set challenge preference
Itsi::Server.set_challenge_preference(:tls_alpn01)
```

## Environment Variables

Configure ACME settings using environment variables:

```bash
# Required: Contact email for ACME account
export ITSI_ACME_CONTACT_EMAIL="admin@example.com"

# Optional: Cache directory for certificates
export ITSI_ACME_CACHE_DIR="/var/cache/itsi/acme"

# Optional: ACME directory URL (for testing/staging)
export ITSI_ACME_DIRECTORY_URL="https://acme-staging-v02.api.letsencrypt.org/directory"

# Optional: Custom CA certificate for testing
export ITSI_ACME_CA_PEM_PATH="/path/to/custom-ca.pem"
```

{{< callout type="info" >}}
Environment variables are overridden by explicit configuration in the `acme_certificates` block.
{{< /callout >}}

## Development and Testing

### Staging Environment

Always test with Let's Encrypt staging to avoid rate limits:

```ruby
# For staging/testing
acme_certificates do
  contact_email "admin@example.com"
  directory_url "https://acme-staging-v02.api.letsencrypt.org/directory"
  
  certificate ["staging.example.com"]
end
```

### Environment-Specific Configuration

```ruby
case ENV["RAILS_ENV"]
when "development"
  # Use self-signed certificates in development
  bind "https://localhost:8443"
when "staging"
  # Use Let's Encrypt staging in staging
  acme_certificates do
    contact_email "admin@example.com"
    directory_url "https://acme-staging-v02.api.letsencrypt.org/directory"
    certificate ["staging.example.com"]
  end
  bind "https://staging.example.com"
when "production"
  # Use Let's Encrypt production in production
  acme_certificates do
    contact_email "admin@example.com"
    cache_dir "/var/cache/itsi/acme"
    challenge_preference :http01
    
    certificate ["example.com", "www.example.com"]
    certificate "api.example.com"
    
    on_certificate_error do |event|
      send_alert("Certificate error: #{event[:error]}")
    end
  end
  bind "https://example.com"
  bind "https://api.example.com"
end
```

## Integration Examples

### Docker Deployment

```ruby
# Itsi.rb for Docker
acme_certificates do
  contact_email ENV.fetch("ACME_EMAIL")
  cache_dir "/app/acme-cache"  # Mount this as a volume
  challenge_preference :http01
  
  certificate ENV.fetch("DOMAINS").split(",")
  
  on_certificate_error do |event|
    # Log to stdout for container logging
    puts "ACME_ERROR: #{event[:error]}"
  end
end

bind "https://0.0.0.0:443"
bind "http://0.0.0.0:80"  # For HTTP-01 challenges
```

### Kubernetes with Ingress

```ruby
# For services behind Kubernetes ingress
acme_certificates do
  contact_email "admin@example.com"
  cache_dir "/etc/ssl/acme"
  challenge_preference :http01
  
  certificate ["api.example.com"]
  
  on_certificate_issued do |event|
    # Update Kubernetes TLS secret
    update_kubernetes_tls_secret(event[:domains], event[:certificate])
  end
end
```

### CDN Integration

```ruby
# For servers behind CloudFlare or similar CDN
acme_certificates do
  contact_email "admin@example.com"
  challenge_preference :http01  # CDN forwards HTTP traffic
  
  certificate ["example.com", "www.example.com"]
  
  on_certificate_issued do |event|
    # Update CDN with new certificate
    update_cloudflare_certificate(event[:domains], event[:certificate])
  end
end
```

## Best Practices

{{< callout type="info" >}}
**Cache Directory**: Always configure a persistent cache directory in production to avoid re-requesting certificates on server restarts.
{{< /callout >}}

{{< callout type="warn" >}}
**Rate Limits**: Let's Encrypt enforces strict rate limits (50 certificates per registered domain per week). Always test with staging first.
{{< /callout >}}

{{< callout type="info" >}}
**Domain Accessibility**: Ensure your server is accessible from the internet on the domains you're requesting certificates for.
{{< /callout >}}

{{< callout type="warn" >}}
**Challenge Type Selection**: Use HTTP-01 for most deployments. Only use TLS-ALPN-01 when port 80 is unavailable or blocked.
{{< /callout >}}

### Security Considerations

1. **File Permissions**: Secure the cache directory with appropriate permissions
2. **Email Verification**: Use a valid email address for ACME account registration
3. **Certificate Rotation**: Let certificates auto-renew rather than managing manually
4. **Monitoring**: Set up alerts for certificate events and expiration

### Performance Tips

1. **Caching**: Use persistent storage for the cache directory
2. **Event Handlers**: Keep event handlers fast and non-blocking
3. **Batch Operations**: Group related domains in the same certificate when possible
4. **Monitoring**: Track certificate metrics and renewal patterns

## Troubleshooting

### Common Issues

**Domain Validation Failed**
```
Error: validation_failed for example.com
```
- Ensure DNS points to your server
- Verify ports 80 (HTTP-01) or 443 (TLS-ALPN-01) are accessible
- Check firewall rules

**Rate Limit Exceeded**
```
Error: rate_limit exceeded
```
- Switch to staging environment for testing
- Wait for rate limit window to reset
- Use fewer certificate requests

**Challenge Type Not Supported**
```
Error: challenge type http01 not supported
```
- Verify your network setup supports the chosen challenge type
- Try switching challenge preference
- Check CDN/proxy configuration

### Debug Configuration

```ruby
acme_certificates do
  contact_email "admin@example.com"
  directory_url "https://acme-staging-v02.api.letsencrypt.org/directory"
  
  on_certificate_event do |event|
    # Log all events for debugging
    puts "ACME Event: #{event.inspect}"
  end
  
  certificate ["test.example.com"]
end
```

### Validation Commands

```ruby
# Check if certificate management is working
after_start do
  # Test API availability
  puts "ACME API available: #{Itsi::Server.respond_to?(:add_certificate)}"
  
  # Check current challenge preference
  puts "Challenge preference: #{Itsi::Server.get_challenge_preference}"
  
  # List existing certificates
  certs = Itsi::Server.list_certificates
  puts "Existing certificates: #{certs.length}"
end
```

## Migration Guide

### From Manual Certificates

If you're currently using manual certificates, migrate gradually:

```ruby
# Phase 1: Configure ACME alongside existing certificates
acme_certificates do
  contact_email "admin@example.com"
  certificate ["new.example.com"]  # Start with new domains
end

bind "https://existing.example.com?cert=/path/to/existing.pem&key=/path/to/existing.key"
bind "https://new.example.com"  # Uses ACME

# Phase 2: Migrate existing domains
# Remove manual certificate configuration
# Add domains to ACME configuration
```

### From Other ACME Clients

If migrating from certbot or similar tools:

1. **Stop existing renewal jobs**: Disable cron jobs or systemd timers
2. **Import existing certificates**: Copy to Itsi's cache directory
3. **Update configuration**: Configure Itsi ACME with same domains
4. **Test renewal**: Verify Itsi can renew existing certificates

## API Reference

### Certificate Management Methods

```ruby
# Add certificate
Itsi::Server.add_certificate(domains, email = nil)
# Returns: true on success, raises exception on error

# Get certificate status
Itsi::Server.certificate_status(domains)
# Returns: Hash with status, domains, email, timestamps

# List all certificates
Itsi::Server.list_certificates
# Returns: Array of certificate hashes

# Renew certificate
Itsi::Server.renew_certificate(domains)
# Returns: true on success, raises exception on error

# Remove certificate
Itsi::Server.remove_certificate(domains)
# Returns: true on success, raises exception on error

# Get/Set challenge preference
Itsi::Server.get_challenge_preference
# Returns: :http01 or :tls_alpn01

Itsi::Server.set_challenge_preference(type)
# type: :http01 or :tls_alpn01
# Returns: true on success

# Set up event handler
Itsi::Server.on_certificate_event do |event|
  # Handle certificate events
end
```

### Event Data Structure

```ruby
{
  type: :issued,           # :issued, :renewed, :error
  domains: ["example.com"], # Array of domain names
  timestamp: Time.now,     # When event occurred
  
  # Additional data based on event type
  expires_at: Time,        # Certificate expiration (:issued, :renewed)
  error: "Error message",  # Error details (:error)
  error_type: :rate_limit, # Error category (:error)
  retry_count: 1          # Retry attempts (:error)
}
```
