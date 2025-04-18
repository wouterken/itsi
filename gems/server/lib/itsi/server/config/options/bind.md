---
title: Bind
url: /options/bind
---

The **Bind** option instructs Itsi on which network interfaces to listen on.
It supports various protocols and formats to allow flexible binding to TCP/IP addresses or Unix sockets, with optional TLS configuration.
You can bind to multiple interfaces at once.

## Bind Formats

You can specify the bind address as a URI. Common formats include:

- **TCP/HTTP:**
  ```ruby
  bind "http://0.0.0.0:3000"
  ```
  Listens on all interfaces on port 3000 using plain HTTP.

- **TCP/HTTPS:**
  ```ruby
  bind "https://0.0.0.0:3000?cert=/path/to/cert.pem&key=/path/to/key.pem"
  ```
  Listens on all interfaces on port 3000 using HTTPS.
  TLS options (e.g. certificates) can be provided via query parameters. You can also use `cert=acme` with additional ACME parameters to enable automatic certificate retrieval.
  E.g.

  ```ruby
  bind "https://0.0.0.0:3000?cert=acme&acme_email=example@example.com&domains=domain1.com,domain2.com"
  ```

- **Unix Socket:**
  ```ruby
  bind "unix:///tmp/itsi.sock"
  ```
  Listens using a Unix domain socket.

- **TLS over Unix Socket:**
  ```ruby
  bind "tls:///tmp/itsi.sock"
  ```
  Listens using a Unix socket while enabling TLS.

## Configuration File

In your configuration file (typically `Itsi.rb`), specify the bind option using the `bind` function.

## Examples

```ruby {filename="Itsi.rb"}
# Bind to all interfaces on port 3000 with HTTP:
bind "http://0.0.0.0:3000"
```

```ruby {filename="Itsi.rb"}
# Bind to all interfaces on port 3000 with HTTPS using certificate files:
bind "https://0.0.0.0:3000?cert=/path/to/cert.pem&key=/path/to/key.pem"
```

```ruby {filename="Itsi.rb"}
# Bind to a Unix socket:
bind "unix:///tmp/itsi.sock"
```

## Command Line

Bind addresses can also be specified from the command line via the `-b` or  `--bind` option:

```bash
itsi -b "http://0.0.0.0:3000" -b "https://0.0.0.0:3001"
```
