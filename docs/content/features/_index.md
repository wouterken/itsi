---
title: Features
type: docs
weight: 1
---

Itsi bundles a slew of essential modern web features into a single, easy-to-use package.
Here's a list of several of the essentials.
Jump straight in to [install Itsi](/getting_started) and [configure it](/configuration) for a deeper dive.

{{< callout >}}
You don't need to use all of the features listed below to benefit from Itsi. E.g.
* Use it *just* as fast, robust and memory efficient Rack server.
* Or as a reverse proxy that allows you to use plain-old Ruby for configuration.

Pick and choose **just** the features that make sense for you.

{{< /callout >}}


## Web Essentials
{{% details title="Compression" closed="true" %}}
* `zstd`, `br`, `gzip` and `deflate` compression.
* Conditional compression based on route, content-type and body size
* Streaming compression
* Serve static precompressed files from the file-system
* gRPC compression (`zlib` and `gzip`)
* See [compression](/middleware/compression)
{{% /details %}}

{{% details title="CORS" closed="true" %}}
* Full control over CORS rules on a location-by-location basis.
* See [cors](/middleware/cors)
{{% /details %}}

{{% details title="ETag and Cache Control" closed="true" %}}
* Weak and Strong eTag support.
* `If-None-Match` and `If-Modified-Since` support.
* Automated etag generation for dynamic content (or forwarding of existing `etags` if present)
* See [etag](/middleware/etag)
{{% /details %}}

{{% details title="Conditional Middleware" closed="true" %}}
* Expressive controls to apply middleware selectively on a request-by-request basis.
* Expressive matching based on route, content-type and body size, hostnames etc.
* A single Itsi process can support simultaneous running of several apps, each with specialized configuration.
* See [location](/middleware/location)
{{% /details %}}

{{% details title="Redirects" closed="true" %}}
* Simple redirect functionality (all of `permanent`, `temporary`, `found`, `moved_permanently`)
* HTTP to HTTPs Redirects
* Dynamic URL Rewriting
* See [redirect](/middleware/redirect)
{{% /details %}}

{{% details title="Reverse Proxy" closed="true" %}}
* Seamlessly Proxy to downstream HTTP services
* Hostname and SNI overrides
* Header overrides
* Multiple backends per host, with load balancing and failover support
* Automatic retries for Idempotent requests
* Configurable error pages
* See [proxy](/middleware/proxy)
{{% /details %}}

{{% details title="Range Requests" closed="true" %}}
* Partial content delivery support, so clients can resume downloads or stream large files efficiently.
* See [static_assets](/middleware/static_assets)
{{% /details %}}

{{% details title="Static File Server" closed="true" %}}
Efficiently serves static assets with proper content types and caching headers.
* Auto index generation
* Configurable in-memory caching for faster serving of small and frequently accessed files.
* Auto try `.html` extension (for cleaner paths)
* Configurable fallback behaviour (e.g. route request misses to an index.html for SPAs)
* See [static_assets](/middleware/static_assets)
{{% /details %}}

{{% details title="Multiple Binds" closed="true" %}}
* Itsi can listen on multiple IP addresses or ports simultaneously for flexible deployment.
* Unix socket binds (both plain-text and TLS) are supported.
* See [bind](/options/bind).
{{% /details %}}

## DevOps
{{% details title="File Watcher & Live Reloading" closed="true" %}}
* Monitors file changes and to automatically reloads configuration or content.
* Use custom watchers to e.g. trigger frontend builds on file changes.
* See [local_development](/getting_started/local_development).
{{% /details %}}

{{% details title="LSP and shell completion support" closed="true" %}}
* The bundled RubyLSP addon provides rich inline documentation and hover support when editing `Itsi.rb` files.
* Shell completion support (just add `eval "$(itsi --install-completions)"` to the bottom of your shell init file)
* See [local_development](/getting_started/local_development).
{{% /details %}}

{{% details title="Status Reporting" closed="true" %}}
* Send SIGUSR2 to trigger detailed status report across all Itsi processes.
* See [signals](/getting_started/signals).
{{% /details %}}

{{% details title="Granular Logging" closed="true" %}}
* Support logging using plain-text and structured `JSON` formats.
* Support `STDOUT`, file-system and combined log sinks.
* Apply selective log levels for specific log targets only.
* Configurable request logging middleware, with custom log templates.
* See [log_responses](/middleware/log_responses) & [logging](/getting_started/logging).
{{% /details %}}

{{% details title="Hot Config Reloads & Config File Validation" closed="true" %}}
* Zero-downtime config file reloads.
* Phased restart support when running in `cluster` mode
* Config file testing and dry-run functionality
* See [hot_reloads](/getting_started/hot_reloads).
{{% /details %}}

{{% details title="Configurable Error Responses" closed="true" %}}
* Provide your own exception responses (HTML and JSON) for all common exception scenarios, or simply rely on the light-weight defaults.
* See [error_responses](/middleware/error_responses).
{{% /details %}}

{{% details title="Management Signals" closed="true" %}}
* Use a full suite of Unix signals to control your live Itsi cluster.
* Add or remove workers on the fly, reload config, generate status reports etc.
* See [signals](/getting_started/signals).
{{% /details %}}

## Security
{{% details title="JWT/API Key/Basic Auth" closed="true" %}}
* Apply common authentication patterns at the middleware layer.
* API Key (`bcrypt`, `argon2`, `sha256`, `sha512`)
* JWT (`hs256`, `HS384`, `HS512`, `RS256`, `RS384`, `RS512`, `ES256`, `ES384`, `PS256`, `PS384`, `PS512`)
* Basic Auth (`bcrypt`, `argon2`, `sha256`, `sha512`)

Itsi also comes bundled with a passfile generator, to help you manage your password hashes effectively.

* See [auth_jwt](/middleware/auth_jwt), [auth_api_key](/middleware/auth_api_key), [auth_basic](/middleware/auth_basic) and [passfile](/getting_started/passfile).
{{% /details %}}

{{% details title="Automatic Let's Encrypt Certificates" closed="true" %}}
* Automated provisioning of Let's Encrypt certificates.
* File system caching of certificate data to avoid excessive API calls.
* Supports usage of subject alternative names (SANs) for certificates that span multiple domains/sub-domains.
* See [bind](/options/bind).
{{% /details %}}

{{% details title="Automatic Development Certificates" closed="true" %}}
* Easily mirror your production SSL set-up in Development
* Custom local CA generation (add this CA cert to your trusted root certificates for warning-less SSL during local development)
* See [bind](/options/bind).
{{% /details %}}

{{% details title="(Distributed) Rate Limiting" closed="true" %}}
* Combine any number of configurable rate limits
* Support for a `Redis` backend for distributed rate limiting (falls back to in-memory backend)
* In-memory backend for simple setups and local development.
* See [rate_limit](/middleware/rate_limit).
{{% /details %}}

{{% details title="Allow & Deny Lists" closed="true" %}}
* IP Allow lists to limit access to a specific set of IP addresses or blocks.
* IP Deny lists to block access from specific IP addresses or blocks.
* See [allow_list](/middleware/allow_list) & [deny_list](/middleware/deny_list).
{{% /details %}}

{{% details title="Intrusion Protection" closed="true" %}}
* Automatically scan request paths and headers for known malicious patterns
* Configurable ban rules to block offenders for a specified duration.
* See [intrusion_protection](/middleware/intrusion_protection)
{{% /details %}}

{{% details title="Slowhttp attack prevention" closed="true" %}}
* Protections against several slowhttp attacks (e.g. Slowloris, Slowbody), through header and request timeouts and maximum request body sizes.
* See [max_body](/options/max_body), [request_timeout](/options/request_timeout) and [header_read_timeout](/options/header_read_timeout)
{{% /details %}}

{{% details title="CSP Reporting" closed="true" %}}
* Simple configuration for enabling CSP headers.
* Support for hosting a CSP reporting endpoint to track violations of CSPs running in reporting only mode.
* See [CSP](/middleware/csp)
{{% /details %}}

## Protocols & Standards
{{% details title="HTTP2" closed="true" %}}
* Benefit from connection multiplexing by using http2 all the way from client to app/file server.
* `Itsi`'s  underlying HTTP1 and 2 implementations are provided directly by [hyper](https://github.com/hyperium/hyper). Itsi simply exposes these existing capabilities. This means that once [h3](https://hyper.rs/contrib/roadmap/#http3) lands in Hyper - we'll get it in Itsi too!
{{% /details %}}

{{% details title="Rack Server" closed="true" %}}
* Rack compliant. Itsi plays nicely with your existing Rack-based applications and middleware.
* See [run](/middleware/run) and [rack_file](/middleware/rack_file)
{{% /details %}}

{{% details title="gRPC Server" closed="true" %}}
* Itsi is compatible with ruby `grpc` service handlers and can
replace the [official Ruby gRPC server implementation](https://github.com/grpc/grpc/blob/master/src/ruby/README.md) for a free performance boost!
* Consider implementing non-blocking IO to further enhance performance.
* Support for gRPC server reflection (use with tools like evans and Postman for easy service discovery)
* Support for gzip and zlib compression
* See [grpc](/middleware/grpc)
{{% /details %}}

{{% details title="gRPC+REST compatibility mode" closed="true" %}}
* Itsi provides a `gRPC+REST` compatibility layer for easy reuse of gRPC endpoints by clients and environments that are not gRPC capable. Invoke unidirectional and streaming endpoints using plain-old JSON.
* See [grpc](/middleware/grpc)
{{< callout type="warn" >}}
Note: This is not the same as [gRPC with Json](https://grpc.io/blog/grpc-with-json/), which swaps out protobuf for JSON but still relies on gRPC's underlying framing mechanics.
{{< /callout >}}
{{% /details %}}

{{% details title="WebSockets" closed="true" %}}
* WebSocket support for Rack apps (e.g. [ActionCable](https://guides.rubyonrails.org/action_cable_overview.html))
{{% /details %}}

## Concurrency & Performance
{{% details title="Cluster Mode" closed="true" %}}
* Supports running in a clustered mode, to fully leverage multi-core systems.
* See [workers](/options/workers).
{{% /details %}}

{{% details title="Non-blocking(Fiber Scheduler) Mode" closed="true" %}}
* Support for Ruby’s fiber scheduler for non-blocking concurrency, boosting performance during I/O operations.
* Use Itsi's own high-performance build-in [Fiber scheduler](/itsi_scheduler), or if your prefer you can bring your own!
* See [fiber_scheduler](/options/fiber_scheduler).
{{% /details %}}

{{% details title="Hybrid Blocking/Non-Blocking Mode" closed="true" %}}
* `Itsi` allows you to split endpoints between using a Fiber scheduler versus running using the traditional blocking IO model. This allows you to dip your toes into the waters of Ruby's new non-blocking IO, without having to port an entire application at once!
* See [scheduler_threads](/options/scheduler_threads).
{{% /details %}}

{{% details title="Non-blocking by design" closed="true" %}}
* Itsi is underpinned by [hyper](https://hyper.rs/) and [tokio](https://tokio.rs/) and as such is fundamentally an evented, non-blocking server. Whether you're proxying, serving large files, or delegating to Ruby endpoints, `Itsi` remains responsive, even under heavy load.
{{% /details %}}

## Ruby
{{% details title="Preloading" closed="true" %}}
* Preload your Ruby application code before forking to benefit from reduced memory through CoW
* Alternatively, use groups in bundler to target specific gems or dependencies for preloading
* See [preload](/options/preload).
{{% /details %}}

{{% details title="Streaming Bodies" closed="true" %}}
* For both [streaming and enumerable bodies](https://github.com/rack/rack/blob/main/SPEC.rdoc#the-body-), Itsi sends data to the client as soon as it is available. This means modules like [ActionController::Live](https://api.rubyonrails.org/v7.1/classes/ActionController/Live.html) behave as expected, and the minimal buffering keeps `Itsi`'s memory footprint consistently low.
{{% /details %}}

{{% details title="Full & Partial Rack Hijacking" closed="true" %}}
* Itsi supports both [full and partial Rack hijacking](https://github.com/rack/rack/blob/main/SPEC.rdoc#hijacking-). Even over HTTP2!
{{< callout type="warn" >}}
By design, Full hijacking assumes you are writing a raw HTTP1 response directly to a raw connection stream.
Itsi's support for full hijack over HTTP2 is similar to what you would see if running a dedicated reverse proxy in front of a Ruby app.
Itsi translates that request from HTTP1 to HTTP2, in real-time, allowing full hijacking endpoints to write HTTP1 and the client to receive HTTP2.
{{< /callout >}}
{{% /details %}}

{{% details title="Sendfile" closed="true" %}}
* Itsi allows Ruby apps to set a `X-Sendfile` header to enable efficient, streaming file transfers, outside of Ruby, via fast native code.
* See [run](/middleware/run) and [rack_file](/middleware/rack_file).

{{< callout type="info" >}}
Note that despite the header being named `X-Sendfile`, Itsi does not use the Sendfile system call, instead delegating the efficient streaming to Tokio's native asynchronous file streaming capabilities.
{{< /callout >}}

{{% /details %}}

{{% details title="Graceful Memory Limits" closed="true" %}}
* Itsi allows you to specify memory limits for Ruby processes. When the limit is reached, Itsi gracefully terminates the process and also invokes a dedicated `after_memory_threshold_reached` callback,
so that you can log the event for further analysis.
* See [worker_memory_limit](/options/worker_memory_limit) and  [after_memory_threshold_reached](/options/after_memory_threshold_reached).
{{% /details %}}

{{% details title="OOB GC" closed="true" %}}
* Itsi can be configured to periodically trigger GC every N idle periods (where an idle period is defined as a time where no requests are currently queued).
* Periodic triggering of GC outside of the request flow can help reduce the impact of GC on latency.
* See [oob_gc_threshold](/options/oob_gc_threshold)
{{% /details %}}

{{% details title="'Rackless' Ruby Apps" closed="true" %}}
* Itsi allows definition of ultra-light-weight Ruby web-apps, using plain old functions and procs.
* For simple endpoints this barebones option can provide a substantial increase in throughput over a Rack request (primarily by avoiding allocating the env hash and response array)
* See [endpoint](/middleware/endpoint)
{{% /details %}}
