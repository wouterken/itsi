---
title: Acknowledgements
type: docs
prev: itsi_scheduler/
sidebar:
  exclude: true
---

Itsi has a long list of **critical** dependencies and **strong** influences.


## Key Dependencies
* [hyper](https://hyper.rs/)
Hyper is a fast, correct and memory-safe HTTP1 & 2 Implementation in Rust.
It is an absolutely essential component of Itsi.

* [tokio](https://tokio.rs/)
Tokio is a fast and featureful asynchronous runtime for Rust.
It is the backbone of *all* asynchronous IO in Itsi.

* and many more [essential](https://github.com/wouterken/itsi/blob/main/crates/itsi_server/Cargo.toml) Rust crates!

* [hugo](https://gohugo.io/) and [hextra](https://imfing.github.io/hextra/) - generating *this very page* and allowing me to put together
this website with minimal effort.


## Inspiration
* [Puma](https://puma.io/) is a long-standing industry heavy-weight and the current leading choice of Web Server in the Ruby ecosystem.
It's mature, stable and rock solid. Many features and interfaces of Itsi have been inspired by Puma.

* [NGINX](https://nginx.org/) is a popular open-source web server and reverse proxy server.
It's highly scalable and a great choice for high-traffic websites.
Many of Itsi's proxy and static file server design decisions and features have been inspired by their NGINX equivalents.

* The [Async](https://github.com/socketry/async) ecosystem and [Falcon](https://github.com/socketry/falcon), championed by [@ioaquatix](https://github.com/ioquatix) - a fellow Kiwi.
These tools and efforts in driving forward Ruby's cooperative multitasking have been a great inspiration and source of learning for Itsi's async IO design.

* [Iodine](https://github.com/boazsegev/iodine), [Agoo](https://github.com/ohler55/agoo). Two class-leading options when it comes to blazing fast Ruby servers written in native code.

* [Caddy](https://caddyserver.com/)
An open-source web server and reverse proxy server, with a huge feature set, great performance, and arguably the first tool to popularize automated ACME certificate management.

* [RubyLSP](https://shopify.github.io/ruby-lsp/)
An open-source language server for Ruby, built by engineers at Shopify making it *easy* to significantly enhance the Itsi developer experience.
