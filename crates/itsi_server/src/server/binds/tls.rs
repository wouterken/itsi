use base64::{engine::general_purpose, Engine as _};
use itsi_acme::{AcmeAcceptor, AcmeConfig, AcmeState, Http01Handler, ResolvesServerCertAcme};
use itsi_error::Result;
use itsi_tracing::{error, info};
use locked_dir_cache::LockedDirCache;
use parking_lot::{Mutex as ParkingMutex, RwLock as ParkingRwLock};
use rcgen::ExtendedKeyUsagePurpose;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose,
    SanType,
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
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};
use tokio::runtime::Builder as RuntimeBuilder;
use tokio::sync::{mpsc, watch};
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor};

use crate::env::{
    ITSI_ACME_CACHE_DIR, ITSI_ACME_CA_PEM_PATH, ITSI_ACME_CONTACT_EMAIL, ITSI_ACME_DIRECTORY_URL,
    ITSI_LOCAL_CA_DIR,
};

mod locked_dir_cache;

#[derive(Debug, Clone)]
pub struct ManagedTlsDomainStatus {
    pub domain: String,
    pub status: String,
    pub last_error: Option<String>,
}

#[derive(Clone)]
struct DynamicAcmeConfigTemplate {
    client_config: Arc<ClientConfig>,
    directory_url: String,
    contact: Vec<String>,
    cache_dir: String,
}

impl DynamicAcmeConfigTemplate {
    fn state_for_domain(
        &self,
        domain: &str,
        resolver: Arc<ResolvesServerCertAcme>,
        http01_handler: Arc<Http01Handler>,
        http01_enabled: bool,
    ) -> AcmeState<Error> {
        let state = AcmeConfig::new([domain])
            .contact(self.contact.clone())
            .cache(LockedDirCache::new(self.cache_dir.clone()))
            .directory(&self.directory_url)
            .client_tls_config(self.client_config.clone());
        let mut state = AcmeState::new_with_resolver(
            state,
            resolver,
            http01_handler,
            Some(domain.to_string()),
        );
        state.set_http01_enabled(http01_enabled);
        state
    }
}

enum DynamicAcmeCommand {
    Register(String),
    Unregister(String),
    Shutdown,
}

#[derive(Clone)]
pub struct DynamicAcmeManager {
    inner: Arc<DynamicAcmeManagerInner>,
}

struct DynamicAcmeManagerInner {
    resolver: Arc<ResolvesServerCertAcme>,
    http01_registry: Arc<ParkingRwLock<HashMap<String, Arc<Http01Handler>>>>,
    statuses: Arc<ParkingRwLock<HashMap<String, ManagedTlsDomainStatus>>>,
    http01_enabled: Arc<AtomicBool>,
    initialized: AtomicBool,
    initial_domains: Vec<String>,
    command_tx: mpsc::UnboundedSender<DynamicAcmeCommand>,
    thread_handle: ParkingMutex<Option<JoinHandle<()>>>,
}

impl DynamicAcmeManager {
    fn new(template: DynamicAcmeConfigTemplate, initial_domains: Vec<String>) -> Self {
        let resolver = ResolvesServerCertAcme::new();
        let http01_handler = Arc::new(Http01Handler::new());
        let http01_registry = Arc::new(ParkingRwLock::new(HashMap::new()));
        let statuses = Arc::new(ParkingRwLock::new(HashMap::new()));
        let http01_enabled = Arc::new(AtomicBool::new(false));
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();

        let resolver_clone = resolver.clone();
        let http01_handler_clone = http01_handler.clone();
        let http01_registry_clone = http01_registry.clone();
        let statuses_clone = statuses.clone();
        let http01_enabled_clone = http01_enabled.clone();

        let thread_handle = std::thread::spawn(move || {
            let runtime = RuntimeBuilder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build dynamic ACME runtime");
            runtime.block_on(async move {
                let mut cancellations: HashMap<String, watch::Sender<bool>> = HashMap::new();

                while let Some(command) = command_rx.recv().await {
                    match command {
                        DynamicAcmeCommand::Register(domain) => {
                            let domain = domain.to_ascii_lowercase();
                            if cancellations.contains_key(&domain) {
                                continue;
                            }

                            statuses_clone.write().insert(
                                domain.clone(),
                                ManagedTlsDomainStatus {
                                    domain: domain.clone(),
                                    status: "pending".to_string(),
                                    last_error: None,
                                },
                            );
                            http01_registry_clone
                                .write()
                                .insert(domain.clone(), http01_handler_clone.clone());

                            let (cancel_tx, mut cancel_rx) = watch::channel(false);
                            cancellations.insert(domain.clone(), cancel_tx);

                            let resolver = resolver_clone.clone();
                            let http01_handler = http01_handler_clone.clone();
                            let registry = http01_registry_clone.clone();
                            let statuses = statuses_clone.clone();
                            let template = template.clone();
                            let enabled = http01_enabled_clone.clone();
                            let task_domain = domain.clone();

                            tokio::spawn(async move {
                                statuses.write().insert(
                                    task_domain.clone(),
                                    ManagedTlsDomainStatus {
                                        domain: task_domain.clone(),
                                        status: "issuing".to_string(),
                                        last_error: None,
                                    },
                                );
                                let mut state = template.state_for_domain(
                                    &task_domain,
                                    resolver.clone(),
                                    http01_handler,
                                    enabled.load(Ordering::SeqCst),
                                );

                                loop {
                                    tokio::select! {
                                        changed = cancel_rx.changed() => {
                                            if changed.is_ok() && *cancel_rx.borrow() {
                                                resolver.remove_auth_key(&task_domain);
                                                resolver.remove_cert_for_domain(&task_domain);
                                                registry.write().remove(&task_domain);
                                                statuses.write().remove(&task_domain);
                                                break;
                                            }
                                        }
                                        event = futures::StreamExt::next(&mut state) => {
                                            match event {
                                                Some(Ok(_)) => {
                                                    let mut statuses = statuses.write();
                                                    if let Some(status) = statuses.get_mut(&task_domain) {
                                                        status.status = "active".to_string();
                                                        status.last_error = None;
                                                    }
                                                }
                                                Some(Err(err)) => {
                                                    let mut statuses = statuses.write();
                                                    if let Some(status) = statuses.get_mut(&task_domain) {
                                                        status.status = "failed".to_string();
                                                        status.last_error = Some(err.to_string());
                                                    }
                                                }
                                                None => break,
                                            }
                                        }
                                    }
                                }
                            });
                        }
                        DynamicAcmeCommand::Unregister(domain) => {
                            let domain = domain.to_ascii_lowercase();
                            if let Some(cancel_tx) = cancellations.remove(&domain) {
                                let _ = cancel_tx.send(true);
                            } else {
                                http01_registry_clone.write().remove(&domain);
                                resolver_clone.remove_auth_key(&domain);
                                resolver_clone.remove_cert_for_domain(&domain);
                                statuses_clone.write().remove(&domain);
                            }
                        }
                        DynamicAcmeCommand::Shutdown => {
                            for (_, cancel_tx) in cancellations.drain() {
                                let _ = cancel_tx.send(true);
                            }
                            break;
                        }
                    }
                }
            });
        });

        Self {
            inner: Arc::new(DynamicAcmeManagerInner {
                resolver,
                http01_registry,
                statuses,
                http01_enabled,
                initialized: AtomicBool::new(false),
                initial_domains,
                command_tx,
                thread_handle: ParkingMutex::new(Some(thread_handle)),
            }),
        }
    }

    pub fn resolver(&self) -> Arc<ResolvesServerCertAcme> {
        self.inner.resolver.clone()
    }

    pub fn set_http01_enabled(&self, enabled: bool) {
        self.inner.http01_enabled.store(enabled, Ordering::SeqCst);
    }

    pub fn initialize_domains(&self) {
        if self.inner.initialized.swap(true, Ordering::SeqCst) {
            return;
        }

        for domain in &self.inner.initial_domains {
            self.register_domain(domain.clone());
        }
    }

    pub fn register_domain(&self, domain: String) {
        let _ = self
            .inner
            .command_tx
            .send(DynamicAcmeCommand::Register(domain));
    }

    pub fn unregister_domain(&self, domain: &str) {
        let _ = self
            .inner
            .command_tx
            .send(DynamicAcmeCommand::Unregister(domain.to_string()));
    }

    pub fn http01_response(&self, host: &str, path: &str) -> Option<String> {
        self.inner
            .http01_registry
            .read()
            .get(host)
            .and_then(|handler| handler.handle_challenge_request(path))
    }

    pub fn statuses(&self) -> Vec<ManagedTlsDomainStatus> {
        let mut statuses = self
            .inner
            .statuses
            .read()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        statuses.sort_by(|a, b| a.domain.cmp(&b.domain));
        statuses
    }
}

impl Drop for DynamicAcmeManagerInner {
    fn drop(&mut self) {
        let _ = self.command_tx.send(DynamicAcmeCommand::Shutdown);
        if let Some(handle) = self.thread_handle.lock().take() {
            if let Err(err) = handle.join() {
                error!("Dynamic ACME manager thread join failed: {:?}", err);
            }
        }
    }
}

#[derive(Clone)]
pub enum ItsiTlsAcceptor {
    Manual(TlsAcceptor),
    Automatic {
        acme_acceptor: AcmeAcceptor,
        manager: DynamicAcmeManager,
        server_config: Arc<ServerConfig>,
    },
}

impl ItsiTlsAcceptor {
    pub fn manager(&self) -> Option<DynamicAcmeManager> {
        match self {
            ItsiTlsAcceptor::Automatic { manager, .. } => Some(manager.clone()),
            ItsiTlsAcceptor::Manual(_) => None,
        }
    }

    pub fn set_http01_enabled(&self, enabled: bool) {
        if let ItsiTlsAcceptor::Automatic { manager, .. } = self {
            manager.set_http01_enabled(enabled);
        }
    }

    pub fn initialize_domains(&self) {
        if let ItsiTlsAcceptor::Automatic { manager, .. } = self {
            manager.initialize_domains();
        }
    }
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
        .or_else(|| query_params.get("domain").map(|v| vec![v.to_string()]))
        .unwrap_or_default();

    if query_params.get("cert").is_some_and(|c| c == "acme") {
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

        let client_config = if let Ok(ca_pem_path) = &*ITSI_ACME_CA_PEM_PATH {
            let mut root_cert_store = RootCertStore::empty();

            let ca_pem = fs::read(ca_pem_path).expect("failed to read CA pem file");
            let mut ca_reader = BufReader::new(&ca_pem[..]);
            let der_certs: Vec<CertificateDer> = certs(&mut ca_reader)
                .collect::<std::result::Result<Vec<CertificateDer>, _>>()
                .map_err(|e| {
                    itsi_error::ItsiError::ArgumentError(format!("Invalid ACME CA Pem path {:?}", e))
                })?;
            root_cert_store.add_parsable_certificates(der_certs);

            Arc::new(
                ClientConfig::builder()
                    .with_root_certificates(root_cert_store)
                    .with_no_client_auth(),
            )
        } else {
            let mut root_store = RootCertStore::empty();
            root_store.extend(
                webpki_roots::TLS_SERVER_ROOTS
                    .iter()
                    .map(|ta| rustls::pki_types::TrustAnchor {
                        subject: ta.subject.clone(),
                        subject_public_key_info: ta.subject_public_key_info.clone(),
                        name_constraints: ta.name_constraints.clone(),
                    }),
            );
            Arc::new(
                ClientConfig::builder()
                    .with_root_certificates(root_store)
                    .with_no_client_auth(),
            )
        };

        let manager = DynamicAcmeManager::new(
            DynamicAcmeConfigTemplate {
                client_config,
                directory_url: directory_url.to_string(),
                contact: vec![format!("mailto:{}", acme_contact_email)],
                cache_dir: ITSI_ACME_CACHE_DIR.to_string(),
            },
            domains.clone(),
        );

        let mut rustls_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(manager.resolver());

        rustls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        return Ok(ItsiTlsAcceptor::Automatic {
            acme_acceptor: AcmeAcceptor::new(manager.resolver()),
            manager,
            server_config: Arc::new(rustls_config),
        });
    }
    let (certs, key) = if let (Some(cert_path), Some(key_path)) =
        (query_params.get("cert"), query_params.get("key"))
    {
        // Load from file or Base64
        let certs = load_certs(cert_path);
        let key = load_private_key(key_path);
        (certs, key)
    } else {
        generate_ca_signed_cert(if domains.is_empty() { vec![host.to_owned()] } else { domains })?
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
    ee_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];

    let ee_cert = ee_params.signed_by(&ee_key, &ca_cert, &ca_kp).unwrap();
    let ee_cert_der = ee_cert.der().to_vec();
    let ee_cert = CertificateDer::from(ee_cert_der);

    Ok((
        vec![ee_cert],
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
        let subject_alt_names = vec!["ca.itsi.fyi".to_string(), "localhost".to_string()];
        let mut params = CertificateParams::new(subject_alt_names)?;
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, "ca.itsi.fyi");
        params.distinguished_name = distinguished_name;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature, // useful for OCSP/CRL signing
        ];
        let key_pair = KeyPair::generate()?;
        let cert = params.self_signed(&key_pair)?;

        fs::write(&key_path, key_pair.serialize_pem())?;
        fs::write(&cert_path, cert.pem())?;

        Ok((key_pair.serialize_pem(), cert.pem()))
    }
}
