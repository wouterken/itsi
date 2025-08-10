use crate::services::certificate_service::{
    CertificateEvent, CertificateService, CertificateStatus,
};
use itsi_tracing::info;
use magnus::error::Result;
use magnus::{prelude::*, RArray, RHash, Ruby, Symbol, Value};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Global certificate service instance
static CERTIFICATE_SERVICE: std::sync::OnceLock<Arc<CertificateService>> =
    std::sync::OnceLock::new();

/// Initialize the certificate service
pub fn init_certificate_service() {
    let service = Arc::new(CertificateService::new());
    CERTIFICATE_SERVICE.set(service).ok();
}

/// Get the certificate service instance
fn get_certificate_service() -> Result<Arc<CertificateService>> {
    CERTIFICATE_SERVICE.get().cloned().ok_or_else(|| {
        magnus::Error::new(
            magnus::exception::runtime_error(),
            "Certificate service not initialized",
        )
    })
}

/// Convert Ruby array to Vec<String>
fn ruby_array_to_string_vec(ruby_array: RArray) -> Result<Vec<String>> {
    let mut domains = Vec::new();
    for item in ruby_array.into_iter() {
        let domain: String = String::try_convert(item)?;
        domains.push(domain);
    }
    Ok(domains)
}

/// Convert Vec<String> to Ruby array
fn string_vec_to_ruby_array(ruby: &Ruby, domains: &[String]) -> RArray {
    let array = RArray::new();
    for domain in domains {
        array.push(ruby.str_new(domain)).unwrap();
    }
    array
}

/// Convert CertificateStatus to Ruby hash
fn certificate_status_to_ruby_hash(ruby: &Ruby, status: &CertificateStatus) -> RHash {
    let hash = RHash::new();

    match status {
        CertificateStatus::Pending => {
            hash.aset(ruby.str_new("status"), ruby.str_new("pending"))
                .unwrap();
        }
        CertificateStatus::Active { expires_at } => {
            hash.aset(ruby.str_new("status"), ruby.str_new("active"))
                .unwrap();
            if let Some(expires) = expires_at {
                hash.aset(
                    ruby.str_new("expires_at"),
                    ruby.str_new(&expires.to_rfc3339()),
                )
                .unwrap();
            }
        }
        CertificateStatus::Renewing => {
            hash.aset(ruby.str_new("status"), ruby.str_new("renewing"))
                .unwrap();
        }
        CertificateStatus::Error { message } => {
            hash.aset(ruby.str_new("status"), ruby.str_new("error"))
                .unwrap();
            hash.aset(ruby.str_new("error"), ruby.str_new(message))
                .unwrap();
        }
        CertificateStatus::NotFound => {
            hash.aset(ruby.str_new("status"), ruby.str_new("not_found"))
                .unwrap();
        }
    }

    hash
}

/// Ruby API: Add a certificate for the specified domains
/// Usage: Itsi::Server.add_certificate(['example.com'], 'admin@example.com')
pub fn add_certificate(ruby: &Ruby, domains_arg: Value, email_arg: Value) -> Result<Value> {
    let domains: RArray = RArray::try_convert(domains_arg)?;
    let email: Option<String> = if email_arg.is_nil() {
        None
    } else {
        Some(String::try_convert(email_arg)?)
    };

    let domains_vec = ruby_array_to_string_vec(domains)?;

    if domains_vec.is_empty() {
        return Err(magnus::Error::new(
            magnus::exception::runtime_error(),
            "domains cannot be empty",
        ));
    }

    let service = get_certificate_service()?;

    // For testing purposes, simulate async operation synchronously
    // In a real implementation, this would be properly integrated with the server's async runtime
    match service.add_certificate_sync(domains_vec, email) {
        Ok(_) => {}
        Err(e) => {
            return Err(magnus::Error::new(
                magnus::exception::runtime_error(),
                format!("{}", e),
            ))
        }
    }

    Ok(ruby.qtrue().as_value())
}

/// Ruby API: Remove a certificate for the specified domains
/// Usage: Itsi::Server.remove_certificate(['example.com'])
pub fn remove_certificate(ruby: &Ruby, domains_arg: Value) -> Result<Value> {
    let domains: RArray = RArray::try_convert(domains_arg)?;
    let domains_vec = ruby_array_to_string_vec(domains)?;

    if domains_vec.is_empty() {
        return Err(magnus::Error::new(
            magnus::exception::runtime_error(),
            "domains cannot be empty",
        ));
    }

    let service = get_certificate_service()?;

    // For testing purposes, simulate async operation synchronously
    match service.remove_certificate_sync(&domains_vec) {
        Ok(_) => {}
        Err(e) => {
            return Err(magnus::Error::new(
                magnus::exception::runtime_error(),
                format!("{}", e),
            ))
        }
    }

    Ok(ruby.qtrue().as_value())
}

/// Ruby API: List all managed certificates
/// Usage: Itsi::Server.list_certificates
pub fn list_certificates(ruby: &Ruby) -> Result<Value> {
    let service = get_certificate_service()?;
    let certificates = service.list_certificates();

    let result = RArray::new();

    for cert_info in certificates {
        let cert_hash = RHash::new();
        cert_hash.aset(
            ruby.str_new("domains"),
            string_vec_to_ruby_array(ruby, &cert_info.domains),
        )?;
        cert_hash.aset(
            ruby.str_new("acme_email"),
            ruby.str_new(&cert_info.acme_email),
        )?;
        cert_hash.aset(
            ruby.str_new("created_at"),
            ruby.str_new(&cert_info.created_at.to_rfc3339()),
        )?;
        cert_hash.aset(
            ruby.str_new("last_updated"),
            ruby.str_new(&cert_info.last_updated.to_rfc3339()),
        )?;
        cert_hash.aset(
            ruby.str_new("status"),
            certificate_status_to_ruby_hash(ruby, &cert_info.status),
        )?;

        result.push(cert_hash)?;
    }

    Ok(result.as_value())
}

/// Ruby API: Renew a certificate for the specified domains
/// Usage: Itsi::Server.renew_certificate(['example.com'])
pub fn renew_certificate(ruby: &Ruby, domains_arg: Value) -> Result<Value> {
    let domains: RArray = RArray::try_convert(domains_arg)?;
    let domains_vec = ruby_array_to_string_vec(domains)?;

    if domains_vec.is_empty() {
        return Err(magnus::Error::new(
            magnus::exception::runtime_error(),
            "domains cannot be empty",
        ));
    }

    let service = get_certificate_service()?;

    // For testing purposes, simulate async operation synchronously
    match service.renew_certificate_sync(&domains_vec) {
        Ok(_) => {}
        Err(e) => {
            return Err(magnus::Error::new(
                magnus::exception::runtime_error(),
                format!("{}", e),
            ))
        }
    }

    Ok(ruby.qtrue().as_value())
}

/// Ruby API: Get certificate status for the specified domains
/// Usage: Itsi::Server.certificate_status(['example.com'])
pub fn certificate_status(ruby: &Ruby, domains_arg: Value) -> Result<Value> {
    let domains: RArray = RArray::try_convert(domains_arg)?;
    let domains_vec = ruby_array_to_string_vec(domains)?;

    if domains_vec.is_empty() {
        return Err(magnus::Error::new(
            magnus::exception::runtime_error(),
            "domains cannot be empty",
        ));
    }

    let service = get_certificate_service()?;
    let status = service.certificate_status(&domains_vec);

    Ok(certificate_status_to_ruby_hash(ruby, &status).as_value())
}

/// Ruby API: Set up certificate event callback
/// Usage: Itsi::Server.on_certificate_event()
pub fn on_certificate_event(ruby: &Ruby) -> Result<Value> {
    // For now, just set up event logging without tokio spawn
    // TODO: Implement proper Ruby callback handling with thread safety
    let service = get_certificate_service()?;
    let (sender, _receiver) = mpsc::unbounded_channel::<CertificateEvent>();
    service.set_event_sender(sender);

    // Note: In a real implementation, this would be integrated with the server's event loop
    // For testing purposes, we just set up the sender
    info!("Certificate event handling enabled");

    Ok(ruby.qtrue().as_value())
}

/// Ruby API: Set ACME challenge preference
/// Usage: Itsi::Server.set_challenge_preference(:http01) # or :tls_alpn01
pub fn set_challenge_preference(ruby: &Ruby, preference: Symbol) -> Result<Value> {
    let pref_str: String = preference.name()?.into();

    match pref_str.as_str() {
        "http01" | "tls_alpn01" => {
            info!("Challenge preference set to: {}", pref_str);
            // TODO: Store preference and use it in certificate configuration
            Ok(ruby.qtrue().as_value())
        }
        _ => Err(magnus::Error::new(
            magnus::exception::runtime_error(),
            "Invalid challenge preference. Use :http01 or :tls_alpn01",
        )),
    }
}

/// Ruby API: Get current ACME challenge preference
/// Usage: Itsi::Server.get_challenge_preference
pub fn get_challenge_preference(_ruby: &Ruby) -> Result<Symbol> {
    // TODO: Return actual stored preference
    // For now, default to http01
    Ok(Symbol::new("http01"))
}
