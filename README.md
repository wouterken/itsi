# Itsi
<img src="itsi-server-100.png" alt="Itsi Server" width="80px" style="display: block; margin-left: auto; margin-right: auto;">

> The Serious Web Server, for Serious People

Itsi is a feature-packed, high performance web and application server, with first-class support for Ruby applications.
It's a compliant Rack server with top-tier performance.
It’s also a well-equipped reverse proxy, API gateway, and static file server, controlled by an intuitive and elegant configuration API and DSL.

Itsi is motivated by the belief that:
>*It should be **easy** to share your application on the internet with confidence, without a need for complex configuration or multiple layers of tools.*

Just your application code and *Itsi* working together, inside a single process, to proudly serve your best work on the world wide web.

## Getting Started
For the best introduction to Itsi, you should take a look at the Itsi documentation website.

https://itsi.fyi

## No time for that? Here's a crash course:

### 1. Get Ruby
Make sure you have Ruby installed! If not, look here:
[https://www.ruby-lang.org/en/documentation/installation/](https://www.ruby-lang.org/en/documentation/installation/)


### 2. Itsi

> On Linux?
You'll need at least `build-essential` and `libclang-dev` installed to build Itsi on Linux.
  E.g.
  ```bash
  apt-get install build-essential libclang-dev
  ```

Then, install Itsi using `gem`:
  ```bash
  gem install itsi
  ```

## 3. Run Itsi
Want to serve a Ruby app? Go to a directory containing a `config.ru` file and run:
```
itsi
```

Want to serve static files? Go to a directory containing static files and run:
```
itsi static
```

Want to run and configure a reverse proxy, API Gateway, Static file server, gRPC server, inline endpoints or any combination of these? You'll need to learn a bit more about Itsi's configuration API and DSL.

Run:
```
itsi init
```
to create a new `Itsi.rb` configuration file and start tweaking.

Need help with the Itsi CLI?
```
itsi --help
```
to see some of the essential options.

Prefer learning by doing? Make sure you have [ruby-lsp](https://shopify.github.io/ruby-lsp/) installed, and then let the LSP show
you the right set of configuration options available inside `Itsi.rb`, from right inside your editor.

Or just go straight to the comprehensive documentation site to see it all!

> https://itsi.fyi/


## Essential Features

> https://itsi.fyi/features


## Configuration

> https://itsi.fyi/configuration

## F.A.Qs

> https://itsi.fyi/faqs

### Looking for Itsi Scheduler? Find it here:

Docs:
> https://itsi.fyi/itsi_scheduler

Source Code:
> https://github.com/wouterken/itsi/blob/main/gems/scheduler
