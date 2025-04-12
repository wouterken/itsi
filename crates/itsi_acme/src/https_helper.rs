use rustls::{pki_types::InvalidDnsNameError, ClientConfig};
use thiserror::Error;

pub use reqwest::Response;

#[derive(Copy, Clone)]
pub enum Method {
    Post,
    Get,
    Head,
}

impl From<Method> for reqwest::Method {
    fn from(m: Method) -> Self {
        match m {
            Method::Post => reqwest::Method::POST,
            Method::Get => reqwest::Method::GET,
            Method::Head => reqwest::Method::HEAD,
        }
    }
}

pub(crate) async fn https(
    client_config: &ClientConfig,
    url: impl AsRef<str>,
    method: Method,
    body: Option<String>,
) -> Result<Response, HttpsRequestError> {
    let method: reqwest::Method = method.into();
    let client = reqwest::ClientBuilder::new()
        .use_preconfigured_tls(client_config.clone())
        .build()?;
    let mut request = client.request(method, url.as_ref());
    if let Some(body) = body {
        request = request
            .body(body)
            .header("Content-Type", "application/jose+json");
    }

    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(HttpsRequestError::Non2xxStatus {
            status_code: status.into(),
            body: response.text().await?,
        });
    }
    Ok(response)
}

impl From<reqwest::Error> for HttpsRequestError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e.into())
    }
}

#[derive(Error, Debug)]
pub enum HttpsRequestError {
    #[error("io error: {0:?}")]
    Io(#[from] std::io::Error),
    #[error("invalid dns name: {0:?}")]
    InvalidDnsName(#[from] InvalidDnsNameError),
    #[error("http error: {0:?}")]
    Http(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("non 2xx http status: {status_code} {body:?}")]
    Non2xxStatus { status_code: u16, body: String },
    #[error("could not determine host from url")]
    UndefinedHost,
}
