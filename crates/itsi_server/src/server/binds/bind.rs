use super::{
    bind_protocol::BindProtocol,
    tls::{configure_tls, ItsiTlsAcceptor},
};
use crate::prelude::*;
use itsi_error::ItsiError;
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs},
    path::PathBuf,
    str::FromStr,
};

#[derive(Debug, Clone)]
pub enum BindAddress {
    Ip(IpAddr),
    UnixSocket(PathBuf),
}

impl Default for BindAddress {
    fn default() -> Self {
        BindAddress::Ip(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
    }
}

#[derive(Default, Clone)]
#[magnus::wrap(class = "Itsi::Bind")]
pub struct Bind {
    pub address: BindAddress,
    pub port: Option<u16>, // None for Unix Sockets
    pub protocol: BindProtocol,
    pub tls_config: Option<TlsOptions>,
}

#[derive(Default, Clone)]
pub struct TlsOptions {
    pub host: String,
    pub options: HashMap<String, String>,
}

impl TlsOptions {
    pub fn build_acceptor(&self) -> Result<ItsiTlsAcceptor> {
        configure_tls(&self.host, &self.options)
    }
}

impl Bind {
    pub fn listener_address_string(&self) -> String {
        match &self.address {
            BindAddress::Ip(ip) => format!("tcp://{}:{}", ip.to_canonical(), self.port.unwrap()),
            BindAddress::UnixSocket(path) => {
                format!("unix://{}", path.as_path().to_str().unwrap())
            }
        }
    }
}

impl std::fmt::Debug for Bind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.address {
            BindAddress::Ip(ip) => match self.protocol {
                BindProtocol::Https if self.port == Some(443) => {
                    write!(f, "{}://{}", self.protocol, ip)
                }
                BindProtocol::Http if self.port == Some(80) => {
                    write!(f, "{}://{}", self.protocol, ip)
                }
                _ => match self.port {
                    Some(port) => write!(f, "{}://{}:{}", self.protocol, ip, port),
                    None => write!(f, "{}://{}", self.protocol, ip),
                },
            },
            BindAddress::UnixSocket(path) => {
                write!(f, "{}://{}", self.protocol, path.display())
            }
        }
    }
}

/// We can build a Bind from a string in the format `protocol://host:port?options`
/// E.g.
/// *`https://example.com:443?tls_cert=/path/to/cert.pem&tls_key=/path/to/key.pem`
/// *`unix:///path/to/socket.sock`
/// *`http://example.com:80`
/// *`https://[::]:80`
impl FromStr for Bind {
    type Err = ItsiError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let (protocol, remainder) = if let Some((proto, rest)) = s.split_once("://") {
            (proto.parse::<BindProtocol>()?, rest)
        } else {
            (BindProtocol::Https, s)
        };

        let (url, options) = if let Some((base, options)) = remainder.split_once('?') {
            (base, parse_bind_options(options))
        } else {
            (remainder, HashMap::new())
        };

        let (host, port) = if url.starts_with('[') {
            // IPv6 with brackets `[::]:port`
            if let Some(end) = url.find(']') {
                let host = &url[1..end]; // Extract `::`
                let port = url[end + 1..]
                    .strip_prefix(':')
                    .and_then(|p| p.parse().ok());
                (host, port)
            } else {
                return Err(ItsiError::InvalidInput(
                    "Invalid IPv6 address format".to_owned(),
                ));
            }
        } else if let Some((h, p)) = url.rsplit_once(':') {
            // Check if `h` is an IPv6 address before assuming it's a port
            if h.contains('.') || h.parse::<Ipv4Addr>().is_ok() {
                (h, p.parse::<u16>().ok()) // IPv4 case
            } else if h.parse::<Ipv6Addr>().is_ok() {
                // If it's IPv6, require brackets for port
                return Err(ItsiError::InvalidInput(
                    "IPv6 addresses must use [ ] when specifying a port".to_owned(),
                ));
            } else {
                (h, p.parse::<u16>().ok()) // Treat as a hostname
            }
        } else {
            (url, None)
        };

        let address = if let Ok(ip) = host.parse::<IpAddr>() {
            BindAddress::Ip(ip)
        } else {
            match protocol {
                BindProtocol::Https | BindProtocol::Http => resolve_hostname(host)
                    .map(BindAddress::Ip)
                    .ok_or(ItsiError::ArgumentError(format!(
                        "Failed to resolve hostname {}",
                        host
                    )))?,
                BindProtocol::Unix | BindProtocol::Unixs => BindAddress::UnixSocket(host.into()),
            }
        };

        let port = match protocol {
            BindProtocol::Http => port.or(Some(80)),
            BindProtocol::Https => port.or(Some(443)),
            BindProtocol::Unix => None,
            BindProtocol::Unixs => None,
        };

        let tls_config = match protocol {
            BindProtocol::Http => None,
            BindProtocol::Https => Some(TlsOptions {
                host: host.to_owned(),
                options,
            }),
            BindProtocol::Unix => None,
            BindProtocol::Unixs => Some(TlsOptions {
                host: host.to_owned(),
                options,
            }),
        };
        let bind = Self {
            address,
            port,
            protocol,
            tls_config,
        };
        Ok(bind)
    }
}

fn parse_bind_options(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// Attempts to resolve a hostname into an IP address.
fn resolve_hostname(hostname: &str) -> Option<IpAddr> {
    (hostname, 0)
        .to_socket_addrs()
        .ok()?
        .find_map(|addr| {
            if addr.is_ipv4() {
                Some(addr.ip()) // Prefer IPv4
            } else {
                None
            }
        })
        .or_else(|| {
            (hostname, 0)
                .to_socket_addrs()
                .ok()?
                .map(|addr| addr.ip())
                .next()
        }) // Fallback to IPv4
}
