# WIP

Fork of `tokio-rustls-acme` to fold in auto fallback to HTTP-01 challenge in case ALPN-01
challenge can not be completed (e.g. due to hosting behind a CDN).

Currently only ALPN-01 is supported.

> Original implementation based on https://github.com/n0-computer/tokio-rustls-acme.
> Original implementation based on https://github.com/FlorianUekermann/rustls-acme.
