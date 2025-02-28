use super::tls::configure_tls;
use itsi_error::ItsiError;
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs},
    path::PathBuf,
    str::FromStr,
};
use tokio_rustls::rustls::ServerConfig;

// When we bind our server,
// we bind to an Address (IPv4, IPV6 or Unix Socket), Port,
// With a protocol.
// If the protocol is HTTPS we'll configure generate a TLS ServerConfig.
#[derive(Debug, Default, Clone)]
pub struct Bind {
    pub address: BindAddress,
    pub port: Option<u16>, // None for Unix Sockets
    pub protocol: TransferProtocol,
    pub tls_config: Option<ServerConfig>,
}

#[derive(Debug, Default, Clone)]
pub enum TransferProtocol {
    Http,
    #[default]
    Https,
    Unix,
}

impl FromStr for TransferProtocol {
    type Err = ItsiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http" => Ok(TransferProtocol::Http),
            "https" => Ok(TransferProtocol::Https),
            "unix" => Ok(TransferProtocol::Unix),
            _ => Err(ItsiError::UnsupportedProtocol(s.to_string())),
        }
    }
}

// Support binding to either IP or Unix Socket
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

impl FromStr for Bind {
    type Err = ItsiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (protocol, remainder) = if let Some((proto, rest)) = s.split_once("://") {
            (proto.parse::<TransferProtocol>()?, rest)
        } else {
            (TransferProtocol::Https, s)
        };

        let (url, options) = if let Some((base, options)) = remainder.split_once('?') {
            (base, parse_bind_options(options))
        } else {
            (s, HashMap::new())
        };

        if let TransferProtocol::Unix = protocol {
            return Ok(Self {
                address: BindAddress::UnixSocket(PathBuf::from(url)),
                protocol: TransferProtocol::Unix,
                port: None,
                tls_config: None,
            });
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
                (h, None) // Treat as a hostname
            }
        } else {
            (url, None)
        };

        let port = port.or(match protocol {
            TransferProtocol::Http => Some(80),
            TransferProtocol::Https => Some(443),
            TransferProtocol::Unix => None,
        });

        let address = if let Ok(ip) = host.parse::<IpAddr>() {
            BindAddress::Ip(ip)
        } else {
            resolve_hostname(host)
                .map(BindAddress::Ip)
                .unwrap_or(BindAddress::Ip(IpAddr::V4(Ipv4Addr::UNSPECIFIED)))
        };

        let tls_config = if let TransferProtocol::Https = protocol {
            Some(configure_tls(host, &options)?)
        } else {
            None
        };

        Ok(Self {
            address,
            port,
            protocol,
            tls_config,
        })
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
        .filter_map(|addr| {
            if addr.is_ipv6() {
                Some(addr.ip()) // Prefer IPv6
            } else {
                None
            }
        })
        .next()
        .or_else(|| {
            (hostname, 0)
                .to_socket_addrs()
                .ok()?
                .map(|addr| addr.ip())
                .next()
        }) // Fallback to IPv4
}
