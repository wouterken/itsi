use base64::{engine::general_purpose, Engine as _};
use itsi_error::Result;
use itsi_tracing::info;
use locked_dir_cache::LockedDirCache;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, SanType,
};
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    ClientConfig, RootCertStore,
};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::{
    collections::HashMap,
    fs,
    io::{BufReader, Error},
    sync::Arc,
};
use tokio::sync::Mutex;
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor};
use tokio_rustls_acme::{AcmeAcceptor, AcmeConfig, AcmeState};

use crate::env::{
    ITSI_ACME_CACHE_DIR, ITSI_ACME_CA_PEM_PATH, ITSI_ACME_CONTACT_EMAIL, ITSI_ACME_DIRECTORY_URL,
    ITSI_LOCAL_CA_DIR,
};

mod locked_dir_cache;

#[derive(Clone)]
pub enum ItsiTlsAcceptor {
    Manual(TlsAcceptor),
    Automatic(
        AcmeAcceptor,
        Arc<Mutex<AcmeState<Error>>>,
        Arc<ServerConfig>,
    ),
}

/// Generates a TLS configuration based on either :
/// * Input "cert" and "key" options (either paths or Base64-encoded strings) or
/// * Performs automatic certificate generation/retrieval. Generated certs use an internal self-signed Isti CA.
///
/// If a non-local host or optional domain parameter is provided,
/// an automated certificate will attempt to be fetched using let's encrypt.
pub fn configure_tls(
    host: &str,
    query_params: &HashMap<String, String>,
) -> Result<ItsiTlsAcceptor> {
    let domains = query_params
        .get("domains")
        .map(|v| v.split(',').map(String::from).collect::<Vec<_>>())
        .or_else(|| query_params.get("domain").map(|v| vec![v.to_string()]));

    if query_params.get("cert").is_some_and(|c| c == "acme") {
        if let Some(domains) = domains {
            let directory_url = &*ITSI_ACME_DIRECTORY_URL;
            info!(
                domains = format!("{:?}", domains),
                directory_url, "Requesting acme cert"
            );
            let acme_contact_email = query_params
                .get("acme_email")
                .map(|s| s.to_string())
                .or_else(|| (*ITSI_ACME_CONTACT_EMAIL).as_ref().ok().map(|s| s.to_string()))
                .ok_or_else(|| itsi_error::ItsiError::ArgumentError(
                    "acme_email query param or ITSI_ACME_CONTACT_EMAIL must be set before you can auto-generate let's encrypt certificates".to_string(),
                ))?;

            let acme_config = AcmeConfig::new(domains)
                .contact([format!("mailto:{}", acme_contact_email)])
                .cache(LockedDirCache::new(&*ITSI_ACME_CACHE_DIR))
                .directory(directory_url);

            let acme_state = if let Ok(ca_pem_path) = &*ITSI_ACME_CA_PEM_PATH {
                let mut root_cert_store = RootCertStore::empty();

                let ca_pem = fs::read(ca_pem_path).expect("failed to read CA pem file");
                let mut ca_reader = BufReader::new(&ca_pem[..]);
                let der_certs: Vec<CertificateDer> = certs(&mut ca_reader)
                    .collect::<std::result::Result<Vec<CertificateDer>, _>>()
                    .map_err(|e| {
                        itsi_error::ItsiError::ArgumentError(format!(
                            "Invalid ACME CA Pem path {:?}",
                            e
                        ))
                    })?;
                root_cert_store.add_parsable_certificates(der_certs);

                let client_config = ClientConfig::builder()
                    .with_root_certificates(root_cert_store)
                    .with_no_client_auth();
                acme_config
                    .client_tls_config(Arc::new(client_config))
                    .state()
            } else {
                acme_config.state()
            };

            let mut rustls_config = ServerConfig::builder()
                .with_no_client_auth()
                .with_cert_resolver(acme_state.resolver());

            rustls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

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
        generate_ca_signed_cert(domains.unwrap_or(vec![host.to_owned()]))?
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
    info!(
        domains = format!("{}", domains.join(", ")),
        "Self signed cert",
    );
    info!(
        "Add {} to your system's trusted cert store to resolve certificate errors.",
        format!("{}/itsi_dev_ca.crt", ITSI_LOCAL_CA_DIR.to_str().unwrap())
    );
    info!("Dev CA path can be overridden by setting env var: `ITSI_LOCAL_CA_DIR`.");
    let (ca_key_pem, ca_cert_pem) = get_or_create_local_dev_ca()?;

    let ca_kp = KeyPair::from_pem(&ca_key_pem).expect("Failed to load CA key");
    let ca_cert = CertificateParams::from_ca_cert_pem(&ca_cert_pem)
        .expect("Failed to parse embedded CA certificate")
        .self_signed(&ca_kp)
        .expect("Failed to self-sign embedded CA cert");

    let ee_key = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    let mut ee_params = CertificateParams::default();

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

fn get_or_create_local_dev_ca() -> Result<(String, String)> {
    let ca_dir = &*ITSI_LOCAL_CA_DIR;
    fs::create_dir_all(ca_dir)?;

    let key_path = ca_dir.join("itsi_dev_ca.key");
    let cert_path = ca_dir.join("itsi_dev_ca.crt");

    if key_path.exists() && cert_path.exists() {
        // Already have a local CA
        let key_pem = fs::read_to_string(&key_path)?;
        let cert_pem = fs::read_to_string(&cert_path)?;

        Ok((key_pem, cert_pem))
    } else {
        let subject_alt_names = vec!["dev.itsi.fyi".to_string(), "localhost".to_string()];
        let mut params = CertificateParams::new(subject_alt_names)?;
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, "Itsi Development CA");
        params.distinguished_name = distinguished_name;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let key_pair = KeyPair::generate()?;
        let cert = params.self_signed(&key_pair)?;

        fs::write(&key_path, key_pair.serialize_pem())?;
        fs::write(&cert_path, cert.pem())?;

        Ok((key_pair.serialize_pem(), cert.pem()))
    }
}
