use base64::{engine::general_purpose, Engine as _};
use itsi_error::Result;
use itsi_tracing::info;
use locked_dir_cache::LockedDirCache;
use rcgen::{CertificateParams, DnType, KeyPair, SanType};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::{
    collections::HashMap,
    env, fs,
    io::{BufReader, Error},
    sync::Arc,
};
use tokio::sync::Mutex;
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor};
use tokio_rustls_acme::{AcmeAcceptor, AcmeConfig, AcmeState};
mod locked_dir_cache;
const ITS_CA_CERT: &str = include_str!("./itsi_ca/itsi_ca.crt");
const ITS_CA_KEY: &str = include_str!("./itsi_ca/itsi_ca.key");

#[derive(Clone)]
pub enum ItsiTlsAcceptor {
    Manual(TlsAcceptor),
    Automatic(
        AcmeAcceptor,
        Arc<Mutex<AcmeState<Error>>>,
        Arc<ServerConfig>,
    ),
}

// Generates a TLS configuration based on either :
// * Input "cert" and "key" options (either paths or Base64-encoded strings) or
// * Performs automatic certificate generation/retrieval. Generated certs use an internal self-signed Isti CA.
// If a non-local host or optional domain parameter is provided,
// an automated certificate will attempt to be fetched using let's encrypt.
pub fn configure_tls(
    host: &str,
    query_params: &HashMap<String, String>,
) -> Result<ItsiTlsAcceptor> {
    let domains = query_params
        .get("domains")
        .map(|v| v.split(',').map(String::from).collect::<Vec<_>>());

    if query_params.get("cert").is_none() || query_params.get("key").is_none() {
        if let Some(domains) = domains {
            let directory_url = env::var("ACME_DIRECTORY_URL")
                .unwrap_or_else(|_| "https://acme-v02.api.letsencrypt.org/directory".to_string());
            info!(
                domains = format!("{:?}", domains),
                directory_url, "Requesting acme cert"
            );
            let acme_state = AcmeConfig::new(domains)
                .contact(["mailto:wc@pico.net.nz"])
                .cache(LockedDirCache::new("./rustls_acme_cache"))
                .directory(directory_url)
                .state();
            let rustls_config = ServerConfig::builder()
                .with_no_client_auth()
                .with_cert_resolver(acme_state.resolver());
            let acceptor = acme_state.acceptor();
            return Ok(ItsiTlsAcceptor::Automatic(
                acceptor,
                Arc::new(Mutex::new(acme_state)),
                Arc::new(rustls_config),
            ));
        }
    }
    let (certs, key) = if let (Some(cert_path), Some(key_path)) =
        (query_params.get("cert"), query_params.get("key"))
    {
        // Load from file or Base64
        let certs = load_certs(cert_path);
        let key = load_private_key(key_path);
        (certs, key)
    } else {
        generate_ca_signed_cert(vec![host.to_owned()])?
    };

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("Failed to build TLS config");

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(ItsiTlsAcceptor::Manual(TlsAcceptor::from(Arc::new(config))))
}

pub fn load_certs(path: &str) -> Vec<CertificateDer<'static>> {
    let data = if let Some(stripped) = path.strip_prefix("base64:") {
        general_purpose::STANDARD
            .decode(stripped)
            .expect("Invalid base64 certificate")
    } else {
        fs::read(path).expect("Failed to read certificate file")
    };

    if data.starts_with(b"-----BEGIN ") {
        let mut reader = BufReader::new(&data[..]);
        let certs_der: Vec<Vec<u8>> = certs(&mut reader)
            .map(|r| {
                r.map(|der| der.as_ref().to_vec())
                    .map_err(itsi_error::ItsiError::from)
            })
            .collect::<Result<_>>()
            .expect("Failed to parse certificate file");
        certs_der
            .into_iter()
            .map(|vec| {
                // Convert the owned Vec<u8> into a CertificateDer and force 'static.
                unsafe { std::mem::transmute(CertificateDer::from(vec)) }
            })
            .collect()
    } else {
        vec![CertificateDer::from(data)]
    }
}

/// Loads a private key from a file or Base64.
pub fn load_private_key(path: &str) -> PrivateKeyDer<'static> {
    let key_data = if let Some(stripped) = path.strip_prefix("base64:") {
        general_purpose::STANDARD
            .decode(stripped)
            .expect("Invalid base64 private key")
    } else {
        fs::read(path).expect("Failed to read private key file")
    };

    if key_data.starts_with(b"-----BEGIN ") {
        let mut reader = BufReader::new(&key_data[..]);
        let keys: Vec<Vec<u8>> = pkcs8_private_keys(&mut reader)
            .map(|r| {
                r.map(|key| key.secret_pkcs8_der().to_vec())
                    .map_err(itsi_error::ItsiError::from)
            })
            .collect::<Result<_>>()
            .expect("Failed to parse private key");
        if !keys.is_empty() {
            return PrivateKeyDer::try_from(keys[0].clone()).unwrap();
        }
    }
    PrivateKeyDer::try_from(key_data).unwrap()
}

pub fn generate_ca_signed_cert(
    domains: Vec<String>,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    info!("Generating New Itsi CA - Self signed Certificate. Use `itsi ca export` to export the CA certificate for import into your local trust store.");

    let ca_kp = KeyPair::from_pem(ITS_CA_KEY).expect("Failed to load embedded CA key");
    let ca_cert = CertificateParams::from_ca_cert_pem(ITS_CA_CERT)
        .expect("Failed to parse embedded CA certificate")
        .self_signed(&ca_kp)
        .expect("Failed to self-sign embedded CA cert");

    let ee_key = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    let mut ee_params = CertificateParams::default();

    info!(
        "Generated certificate will be valid for domains {:?}",
        domains
    );
    use std::net::IpAddr;

    ee_params.subject_alt_names = domains
        .iter()
        .map(|domain| {
            if let Ok(ip) = domain.parse::<IpAddr>() {
                SanType::IpAddress(ip)
            } else {
                SanType::DnsName(domain.clone().try_into().unwrap())
            }
        })
        .collect();

    ee_params
        .distinguished_name
        .push(DnType::CommonName, domains[0].clone());

    ee_params.use_authority_key_identifier_extension = true;

    let ee_cert = ee_params.signed_by(&ee_key, &ca_cert, &ca_kp).unwrap();
    let ee_cert_der = ee_cert.der().to_vec();
    let ee_cert = CertificateDer::from(ee_cert_der);
    let ca_cert = CertificateDer::from(ca_cert.der().to_vec());
    Ok((
        vec![ee_cert, ca_cert],
        PrivateKeyDer::try_from(ee_key.serialize_der()).unwrap(),
    ))
}
