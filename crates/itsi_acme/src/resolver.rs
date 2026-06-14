use crate::acme::ACME_TLS_ALPN_NAME;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug)]
pub struct ResolvesServerCertAcme {
    inner: Mutex<Inner>,
}

#[derive(Debug)]
struct Inner {
    cert: Option<Arc<CertifiedKey>>,
    certs: BTreeMap<String, Arc<CertifiedKey>>,
    auth_keys: BTreeMap<String, Arc<CertifiedKey>>,
}

impl ResolvesServerCertAcme {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner {
                cert: None,
                certs: Default::default(),
                auth_keys: Default::default(),
            }),
        })
    }
    pub fn set_cert(&self, cert: Arc<CertifiedKey>) {
        self.inner.lock().unwrap().cert = Some(cert);
    }
    pub fn set_cert_for_domain(&self, domain: String, cert: Arc<CertifiedKey>) {
        let mut inner = self.inner.lock().unwrap();
        if inner.cert.is_none() {
            inner.cert = Some(cert.clone());
        }
        inner.certs.insert(domain, cert);
    }
    pub fn remove_cert_for_domain(&self, domain: &str) {
        self.inner.lock().unwrap().certs.remove(domain);
    }
    pub fn set_auth_key(&self, domain: String, cert: Arc<CertifiedKey>) {
        self.inner.lock().unwrap().auth_keys.insert(domain, cert);
    }

    pub fn remove_auth_key(&self, domain: &str) {
        self.inner.lock().unwrap().auth_keys.remove(domain);
    }
}

impl ResolvesServerCert for ResolvesServerCertAcme {
    fn resolve(&self, client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        let is_acme_challenge = client_hello
            .alpn()
            .into_iter()
            .flatten()
            .eq([ACME_TLS_ALPN_NAME]);
        if is_acme_challenge {
            match client_hello.server_name() {
                None => {
                    log::debug!("client did not supply SNI");
                    None
                }
                Some(domain) => {
                    let domain = domain.to_owned();
                    let domain: String = AsRef::<str>::as_ref(&domain).into();
                    self.inner.lock().unwrap().auth_keys.get(&domain).cloned()
                }
            }
        } else {
            let inner = self.inner.lock().unwrap();
            match client_hello.server_name() {
                Some(domain) => {
                    let domain = AsRef::<str>::as_ref(&domain);
                    inner.certs.get(domain).cloned().or_else(|| inner.cert.clone())
                }
                None => inner.cert.clone(),
            }
        }
    }
}
