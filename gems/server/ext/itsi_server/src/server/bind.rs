use super::{bind_protocol::BindProtocol, tls::configure_tls};
use itsi_error::ItsiError;
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs},
    path::PathBuf,
    str::FromStr,
};
use tokio_rustls::rustls::ServerConfig;

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

#[derive(Debug, Default, Clone)]
#[magnus::wrap(class = "Itsi::Bind")]
pub struct Bind {
    pub address: BindAddress,
    pub port: Option<u16>, // None for Unix Sockets
    pub protocol: BindProtocol,
    pub tls_config: Option<ServerConfig>,
}

/// We can build a Bind from a string in the format `protocol://host:port?options`
/// E.g.
/// *`https://example.com:443?tls_cert=/path/to/cert.pem&tls_key=/path/to/key.pem`
/// *`unix:///path/to/socket.sock`
/// *`http://example.com:80`
/// *`https://[::]:80`
impl FromStr for Bind {
    type Err = ItsiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
                (h, None) // Treat as a hostname
            }
        } else {
            (url, None)
        };

        let address = if let Ok(ip) = host.parse::<IpAddr>() {
            BindAddress::Ip(ip)
        } else {
            resolve_hostname(host)
                .map(BindAddress::Ip)
                .ok_or(ItsiError::ArgumentError(format!(
                    "Failed to resolve hostname {}",
                    host
                )))?
        };
        let (port, address) = match protocol {
            BindProtocol::Http => (port.or(Some(80)), address),
            BindProtocol::Https => (port.or(Some(443)), address),
            BindProtocol::Unix => (None, BindAddress::UnixSocket(host.into())),
        };

        // Use HTTPs if we've bound to an https://<address> string OR the cert option was manually provided.
        // Otherwise, use plain old HTTP
        let tls_config = match protocol {
            BindProtocol::Http => None,
            BindProtocol::Https => Some(configure_tls(host, &options)?),
            _ if options.contains_key("cert") => Some(configure_tls(host, &options)?),
            _ => None,
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
