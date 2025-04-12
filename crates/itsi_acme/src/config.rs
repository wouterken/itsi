use crate::acme::{
    ExternalAccountKey, LETS_ENCRYPT_PRODUCTION_DIRECTORY, LETS_ENCRYPT_STAGING_DIRECTORY,
};
use crate::caches::{BoxedErrCache, CompositeCache, NoCache};
use crate::{AccountCache, Cache, CertCache};
use crate::{AcmeState, Incoming};
use futures::Stream;
use rustls::{ClientConfig, RootCertStore};
use std::convert::Infallible;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use webpki_roots::TLS_SERVER_ROOTS;

/// Configuration for an ACME resolver.
///
/// The type parameters represent the error types for the certificate cache and account cache.
pub struct AcmeConfig<EC: Debug, EA: Debug = EC> {
    pub(crate) client_config: Arc<ClientConfig>,
    pub(crate) directory_url: String,
    pub(crate) domains: Vec<String>,
    pub(crate) contact: Vec<String>,
    pub(crate) cache: Box<dyn Cache<EC = EC, EA = EA>>,
    pub(crate) eab: Option<ExternalAccountKey>,
}

impl AcmeConfig<Infallible, Infallible> {
    /// Creates a new [AcmeConfig] instance.
    ///
    /// The new [AcmeConfig] instance will initially have no cache, and its type parameters for
    /// error types will be `Infallible` since the cache cannot return an error. The methods to set
    /// a cache will change the error types to match those returned by the supplied cache.
    ///
    /// ```rust
    /// # use tokio_rustls_acme::AcmeConfig;
    /// use tokio_rustls_acme::caches::DirCache;
    /// let config = AcmeConfig::new(["example.com"]).cache(DirCache::new("./rustls_acme_cache"));
    /// ```
    ///
    /// Due to limited support for type parameter inference in Rust (see
    /// [RFC213](https://github.com/rust-lang/rfcs/blob/master/text/0213-defaulted-type-params.md)),
    /// [AcmeConfig::new] is not (yet) generic over the [AcmeConfig]'s type parameters.
    /// An uncached instance of [AcmeConfig] with particular type parameters can be created using
    /// [NoCache].
    ///
    /// ```rust
    /// # use tokio_rustls_acme::AcmeConfig;
    /// use tokio_rustls_acme::caches::NoCache;
    /// # type EC = std::io::Error;
    /// # type EA = EC;
    /// let config: AcmeConfig<EC, EA> = AcmeConfig::new(["example.com"]).cache(NoCache::new());
    /// ```
    ///
    pub fn new(domains: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let mut root_store = RootCertStore::empty();
        root_store.extend(
            TLS_SERVER_ROOTS
                .iter()
                .map(|ta| rustls::pki_types::TrustAnchor {
                    subject: ta.subject.clone(),
                    subject_public_key_info: ta.subject_public_key_info.clone(),
                    name_constraints: ta.name_constraints.clone(),
                }),
        );
        let client_config = Arc::new(
            ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth(),
        );
        AcmeConfig {
            client_config,
            directory_url: LETS_ENCRYPT_STAGING_DIRECTORY.into(),
            domains: domains.into_iter().map(|s| s.as_ref().into()).collect(),
            contact: vec![],
            cache: Box::new(NoCache::new()),
            eab: None,
        }
    }
}

impl<EC: 'static + Debug, EA: 'static + Debug> AcmeConfig<EC, EA> {
    /// Set custom `rustls::ClientConfig` for ACME API calls.
    pub fn client_tls_config(mut self, client_config: Arc<ClientConfig>) -> Self {
        self.client_config = client_config;
        self
    }
    pub fn directory(mut self, directory_url: impl AsRef<str>) -> Self {
        self.directory_url = directory_url.as_ref().into();
        self
    }
    pub fn directory_lets_encrypt(mut self, production: bool) -> Self {
        self.directory_url = match production {
            true => LETS_ENCRYPT_PRODUCTION_DIRECTORY,
            false => LETS_ENCRYPT_STAGING_DIRECTORY,
        }
        .into();
        self
    }
    pub fn domains(mut self, contact: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.domains = contact.into_iter().map(|s| s.as_ref().into()).collect();
        self
    }
    pub fn domains_push(mut self, contact: impl AsRef<str>) -> Self {
        self.domains.push(contact.as_ref().into());
        self
    }

    pub fn external_account_binding(mut self, kid: impl AsRef<str>, key: impl AsRef<[u8]>) -> Self {
        self.eab = Some(ExternalAccountKey::new(kid.as_ref().into(), key.as_ref()));
        self
    }

    /// Provide a list of contacts for the account.
    ///
    /// Note that email addresses must include a `mailto:` prefix.
    pub fn contact(mut self, contact: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.contact = contact.into_iter().map(|s| s.as_ref().into()).collect();
        self
    }

    /// Provide a contact for the account.
    ///
    /// Note that an email address must include a `mailto:` prefix.
    pub fn contact_push(mut self, contact: impl AsRef<str>) -> Self {
        self.contact.push(contact.as_ref().into());
        self
    }

    pub fn cache<C: 'static + Cache>(self, cache: C) -> AcmeConfig<C::EC, C::EA> {
        AcmeConfig {
            client_config: self.client_config,
            directory_url: self.directory_url,
            domains: self.domains,
            contact: self.contact,
            cache: Box::new(cache),
            eab: self.eab,
        }
    }
    pub fn cache_compose<CC: 'static + CertCache, CA: 'static + AccountCache>(
        self,
        cert_cache: CC,
        account_cache: CA,
    ) -> AcmeConfig<CC::EC, CA::EA> {
        self.cache(CompositeCache::new(cert_cache, account_cache))
    }
    pub fn cache_with_boxed_err<C: 'static + Cache>(self, cache: C) -> AcmeConfig<Box<dyn Debug>> {
        self.cache(BoxedErrCache::new(cache))
    }
    pub fn cache_option<C: 'static + Cache>(self, cache: Option<C>) -> AcmeConfig<C::EC, C::EA> {
        match cache {
            Some(cache) => self.cache(cache),
            None => self.cache(NoCache::<C::EC, C::EA>::new()),
        }
    }
    pub fn state(self) -> AcmeState<EC, EA> {
        AcmeState::new(self)
    }
    /// Turn a stream of TCP connections into a stream of TLS connections.
    ///
    /// Specify supported protocol names in `alpn_protocols`, most preferred first. If emtpy (`Vec::new()`), we don't do ALPN.
    pub fn incoming<
        TCP: AsyncRead + AsyncWrite + Unpin,
        ETCP,
        ITCP: Stream<Item = Result<TCP, ETCP>> + Unpin,
    >(
        self,
        tcp_incoming: ITCP,
        alpn_protocols: Vec<Vec<u8>>,
    ) -> Incoming<TCP, ETCP, ITCP, EC, EA> {
        self.state().incoming(tcp_incoming, alpn_protocols)
    }
}
