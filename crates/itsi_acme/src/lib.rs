//! An easy-to-use, async compatible [ACME] client library using [rustls] with [ring].
//! The validation mechanism used is tls-alpn-01, which allows serving acme challenge responses and
//! regular TLS traffic on the same port.
//!
//! Is designed to use the tokio runtime, if you need support for other runtimes take a look
//! at the original implementation [rustls-acme](https://github.com/FlorianUekermann/rustls-acme).
//!
//! No persistent tasks are spawned under the hood and the certificate acquisition/renewal process
//! is folded into the streams and futures being polled by the library user.
//!
//! The goal is to provide a [Let's Encrypt](https://letsencrypt.org/) compatible TLS serving and
//! certificate management using a simple and flexible stream based API.
//!
//! This crate uses [ring] as [rustls]'s backend, instead of [aws-lc-rs]. This generally makes it
//! much easier to compile. If you'd like to use [aws-lc-rs] as [rustls]'s backend, we're open to
//! contributions with the necessary `Cargo.toml` changes and feature-flags to enable you to do so.
//!
//! To use tokio-rustls-acme add the following lines to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! tokio-rustls-acme = "*"
//! ```
//!
//! ## High-level API
//!
//! The high-level API consists of a single stream [Incoming] of incoming TLS connection.
//! Polling the next future of the stream takes care of acquisition and renewal of certificates, as
//! well as accepting TLS connections, which are handed over to the caller on success.
//!
//! ```rust,no_run
//! use tokio::io::AsyncWriteExt;
//! use futures::StreamExt;
//! use tokio_rustls_acme::{AcmeConfig, caches::DirCache};
//! use tokio_stream::wrappers::TcpListenerStream;
//!
//! #[tokio::main]
//! async fn main() {
//!     simple_logger::init_with_level(log::Level::Info).unwrap();
//!
//!     let tcp_listener = tokio::net::TcpListener::bind("[::]:443").await.unwrap();
//!     let tcp_incoming = TcpListenerStream::new(tcp_listener);
//!
//!     let mut tls_incoming = AcmeConfig::new(["example.com"])
//!         .contact_push("mailto:admin@example.com")
//!         .cache(DirCache::new("./rustls_acme_cache"))
//!         .incoming(tcp_incoming, Vec::new());
//!
//!     while let Some(tls) = tls_incoming.next().await {
//!         let mut tls = tls.unwrap();
//!         tokio::spawn(async move {
//!             tls.write_all(HELLO).await.unwrap();
//!             tls.shutdown().await.unwrap();
//!         });
//!     }
//! }
//!
//! const HELLO: &'static [u8] = br#"HTTP/1.1 200 OK
//! Content-Length: 11
//! Content-Type: text/plain; charset=utf-8
//!
//! Hello Tls!"#;
//! ```
//!
//! `examples/high_level.rs` implements a "Hello Tls!" server similar to the one above, which accepts
//! domain, port and cache directory parameters.
//!
//! Note that all examples use the let's encrypt staging directory by default.
//! The production directory imposes strict rate limits, which are easily exhausted accidentally
//! during testing and development.
//! For testing with the staging directory you may open `https://<your domain>:<port>` in a browser
//! that allows TLS connections to servers signed by an untrusted CA (in Firefox click "Advanced..."
//! -> "Accept the Risk and Continue").
//!
//! ## Low-level Rustls API
//!
//! For users who may want to interact with [rustls] or [tokio_rustls]
//! directly, the library exposes the underlying certificate management [AcmeState] as well as a
//! matching resolver [ResolvesServerCertAcme] which implements the [rustls::server::ResolvesServerCert] trait.
//! See the server_low_level example on how to use the low-level API directly with [tokio_rustls].
//!
//! ## Account and certificate caching
//!
//! A production server using the let's encrypt production directory must implement both account and
//! certificate caching to avoid exhausting the let's encrypt API rate limits.
//! A file based cache using a cache directory is provided by [caches::DirCache].
//! Caches backed by other persistence layers may be implemented using the [Cache] trait,
//! or the underlying [CertCache], [AccountCache] traits (contributions welcome).
//! [caches::CompositeCache] provides a wrapper to combine two implementors of [CertCache] and
//! [AccountCache] into a single [Cache].
//!
//! Note, that the error type parameters of the cache carries over to some other types in this
//! crate via the [AcmeConfig] they are added to.
//! If you want to avoid different specializations based on cache type use the
//! [AcmeConfig::cache_with_boxed_err] method to construct the an [AcmeConfig] object.
//!
//!
//! ## The acme module
//!
//! The underlying implementation of an async acme client may be useful to others and is exposed as
//! a module. It is incomplete (contributions welcome) and not covered by any stability
//! promises.
//!
//! ## Special thanks
//!
//! This crate was inspired by the [autocert](https://golang.org/x/crypto/acme/autocert/)
//! package for [Go](https://golang.org).
//!
//! The original implementation of this crate can be found at [FlorianUekermann/rustls-acme](https://github.com/FlorianUekermann/rustls-acme/commits/main), this is just a version focused on supporting only tokio.
//!
//! This crate also builds on the excellent work of the authors of
//! [rustls],
//! [tokio-rustls](https://github.com/tokio-rs/tls/tree/master/tokio-rustls) and many others.
//!
//! [ACME]: https://en.wikipedia.org/wiki/Automatic_Certificate_Management_Environment
//! [ring]: https://github.com/briansmith/ring
//! [rustls]: https://github.com/ctz/rustls
//! [aws-lc-rs]: https://github.com/aws/aws-lc-rs

#![cfg_attr(docsrs, feature(doc_auto_cfg))]

mod acceptor;
pub mod acme;
#[cfg(feature = "axum")]
pub mod axum;
mod cache;
pub mod caches;
mod config;
mod https_helper;
mod incoming;
mod jose;
mod resolver;
mod state;

pub use tokio_rustls;

pub use acceptor::*;
pub use cache::*;
pub use config::*;
pub use incoming::*;
pub use resolver::*;
pub use state::*;
