---
title: on_certificate_event
url: /options/hooks/on_certificate_event
---

Called when ACME certificate lifecycle events occur, such as certificate issuance, renewal, or errors.

## Usage

```ruby {filename=Itsi.rb}
on_certificate_event do |event|
  case event[:type]
  when :issued
    puts "Certificate issued for #{event[:domains].join(', ')}"
  when :renewed
    puts "Certificate renewed for #{event[:domains].join(', ')}"
  when :error
    puts "Certificate error: #{event[:error]}"
  end
end
```

## Event Types

### `:issued`
Triggered when a new certificate is successfully obtained from the ACME provider.

```ruby {filename=Itsi.rb}
on_certificate_event do |event|
  if event[:type] == :issued
    # Send notification
    notify_team("New certificate issued for #{event[:domains].join(', ')}")
    
    # Update monitoring
    update_certificate_monitoring(event[:domains], event[:expires_at])
  end
end
```

### `:renewed`
Triggered when an existing certificate is successfully renewed.

```ruby {filename=Itsi.rb}
on_certificate_event do |event|
  if event[:type] == :renewed
    # Log renewal
    logger.info "Certificate renewed for #{event[:domains].join(', ')}"
    
    # Update external systems
    update_load_balancer_certificates(event[:domains])
  end
end
```

### `:error`
Triggered when certificate operations fail (issuance, renewal, validation, etc.).

```ruby {filename=Itsi.rb}
on_certificate_event do |event|
  if event[:type] == :error
    # Send alert
    send_alert("Certificate error for #{event[:domains].join(', ')}: #{event[:error]}")
    
    # Implement fallback strategy
    fallback_to_backup_certificate(event[:domains])
  end
end
```

## Event Data

All events include the following data:

- `:type` - The event type (`:issued`, `:renewed`, `:error`, etc.)
- `:domains` - Array of domain names affected
- `:timestamp` - When the event occurred

### Additional Data by Event Type

**`:issued` events:**
- `:expires_at` - Certificate expiration timestamp
- `:issued_at` - Certificate issuance timestamp
- `:acme_email` - Email used for ACME account

**`:renewed` events:**
- `:expires_at` - New certificate expiration timestamp
- `:renewed_at` - Renewal timestamp
- `:previous_expires_at` - Previous certificate expiration

**`:error` events:**
- `:error` - Error message
- `:error_type` - Error category (`:rate_limit`, `:validation_failed`, `:network_error`, etc.)
- `:retry_count` - Number of retry attempts made

## Multiple Handlers

You can register multiple certificate event handlers:

```ruby {filename=Itsi.rb}
# Logging handler
on_certificate_event do |event|
  logger.info "Certificate event: #{event[:type]} for #{event[:domains].join(', ')}"
end

# Notification handler
on_certificate_event do |event|
  if event[:type] == :error
    send_notification("Certificate error: #{event[:error]}")
  end
end

# Monitoring handler
on_certificate_event do |event|
  update_metrics("certificate_#{event[:type]}", event[:domains])
end
```

## Common Use Cases

### Notification Integration

```ruby {filename=Itsi.rb}
on_certificate_event do |event|
  case event[:type]
  when :issued, :renewed
    slack_notify(
      "✅ Certificate #{event[:type]} for #{event[:domains].join(', ')}\n" \
      "Expires: #{event[:expires_at]}"
    )
  when :error
    slack_notify(
      "❌ Certificate error for #{event[:domains].join(', ')}\n" \
      "Error: #{event[:error]}"
    )
  end
end
```

### Metrics and Monitoring

```ruby {filename=Itsi.rb}
on_certificate_event do |event|
  # Update Prometheus metrics
  case event[:type]
  when :issued
    certificate_issued_counter.increment(domains: event[:domains])
  when :renewed
    certificate_renewed_counter.increment(domains: event[:domains])
  when :error
    certificate_error_counter.increment(
      domains: event[:domains],
      error_type: event[:error_type]
    )
  end
  
  # Update certificate expiry gauge
  if event[:expires_at]
    certificate_expiry_gauge.set(
      event[:expires_at].to_i,
      domains: event[:domains]
    )
  end
end
```

### Automated Recovery

```ruby {filename=Itsi.rb}
on_certificate_event do |event|
  next unless event[:type] == :error
  
  case event[:error_type]
  when :rate_limit
    # Schedule retry for later
    schedule_certificate_retry(event[:domains], delay: 1.hour)
  when :validation_failed
    # Check DNS configuration
    verify_dns_configuration(event[:domains])
  when :network_error
    # Retry with different challenge type
    if event[:retry_count] < 3
      retry_with_different_challenge(event[:domains])
    end
  end
end
```

### Integration with External Systems

```ruby {filename=Itsi.rb}
on_certificate_event do |event|
  next unless event[:type] == :issued || event[:type] == :renewed
  
  # Update CDN certificates
  update_cloudflare_certificates(event[:domains])
  
  # Update load balancer
  update_nginx_certificates(event[:domains])
  
  # Update container orchestration
  update_kubernetes_tls_secrets(event[:domains])
end
```

## Error Handling

Handle exceptions in your event handlers gracefully:

```ruby {filename=Itsi.rb}
on_certificate_event do |event|
  begin
    # Your event handling logic
    process_certificate_event(event)
  rescue StandardError => e
    # Log error but don't crash the server
    logger.error "Certificate event handler error: #{e.message}"
    logger.error e.backtrace.join("\n")
  end
end
```

## Best Practices

{{< callout type="info" >}}
**Keep Handlers Fast**: Certificate event handlers should complete quickly to avoid blocking certificate operations. For long-running tasks, consider using background jobs.
{{< /callout >}}

{{< callout type="warn" >}}
**Handle Exceptions**: Always wrap your event handling logic in exception handling to prevent certificate operations from failing due to handler errors.
{{< /callout >}}

{{< callout type="info" >}}
**Idempotent Operations**: Design your handlers to be idempotent since they may be called multiple times for the same event in certain failure scenarios.
{{< /callout >}}

{{< callout type="warn" >}}
**Avoid Recursive Operations**: Don't call certificate management functions (like `add_certificate` or `renew_certificate`) from within event handlers, as this can create infinite loops.
{{< /callout >}}