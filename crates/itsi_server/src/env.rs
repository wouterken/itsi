use std::{
    env::{var, VarError},
    path::PathBuf,
    sync::LazyLock,
};

type StringVar = LazyLock<String>;
type MaybeStringVar = LazyLock<Result<String, VarError>>;
type PathVar = LazyLock<PathBuf>;

/// ACME Configuration for auto-generating production certificates
/// *ITSI_ACME_CACHE_DIR* - Directory to store cached certificates
/// so that these are not regenerated every time the server starts
pub static ITSI_ACME_CACHE_DIR: StringVar = LazyLock::new(|| {
    var("ITSI_ACME_CACHE_DIR").unwrap_or_else(|_| "./.rustls_acme_cache".to_string())
});

/// *ITSI_ACME_CONTACT_EMAIL* - Contact Email address to provide to ACME server during certificate renewal
pub static ITSI_ACME_CONTACT_EMAIL: MaybeStringVar =
    LazyLock::new(|| var("ITSI_ACME_CONTACT_EMAIL"));

/// *ITSI_ACME_CA_PEM_PATH* - Optional CA Pem path, used for testing with non-trusted CAs for certifcate generation.
pub static ITSI_ACME_CA_PEM_PATH: MaybeStringVar = LazyLock::new(|| var("ITSI_ACME_CA_PEM_PATH"));

/// *ITSI_ACME_DIRECTORY_URL* - Directory URL to use for ACME certificate generation.
pub static ITSI_ACME_DIRECTORY_URL: StringVar = LazyLock::new(|| {
    var("ITSI_ACME_DIRECTORY_URL")
        .unwrap_or_else(|_| "https://acme-v02.api.letsencrypt.org/directory".to_string())
});

/// *ITSI_ACME_LOCK_FILE_NAME* - Name of the lock file used to prevent concurrent certificate generation.
pub static ITSI_ACME_LOCK_FILE_NAME: StringVar =
    LazyLock::new(|| var("ITSI_ACME_LOCK_FILE_NAME").unwrap_or(".acme.lock".to_string()));

pub static ITSI_LOCAL_CA_DIR: PathVar = LazyLock::new(|| {
    var("ITSI_LOCAL_CA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .expect("Failed to find HOME directory when initializing ITSI_LOCAL_CA_DIR")
                .join(".itsi")
        })
});
