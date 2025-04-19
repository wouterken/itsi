---
title: Features
type: docs
weight: 1
next: /getting_started
---

Itsi bundles a slew of essential modern web features into a single, easy-to-use package.
Here's a list of the essentials.
Or jump straight in to <a target="_blank" href="tsi](/getting_started)">install</a> and <a target="_blank" href="t](/configuration)"> configure</a> for a deeper dive.

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
* See <a target="_blank" href="/middleware/compression">compression</a>
{{% /details %}}

{{% details title="CORS" closed="true" %}}
* Full control over CORS rules on a location-by-location basis.
* See <a target="_blank" href="/middleware/cors">cors</a>
{{% /details %}}

{{% details title="ETag and Cache Control" closed="true" %}}
* Weak and Strong eTag support.
* `If-None-Match` and `If-Modified-Since` support.
* Automated etag generation for dynamic content (or forwarding of existing `etags` if present)
* See <a target="_blank" href="/middleware/etag">etag</a> and <a target="_blank" href="/middleware/cache_control">cache_control</a>
{{% /details %}}

{{% details title="Configurable Middleware" closed="true" %}}
* Expressive controls to apply middleware selectively on a request-by-request basis.
* Expressive matching based on route, content-type and body size, hostnames etc.
* A single Itsi process can support simultaneous running of several apps, each with specialized configuration.
* See <a target="_blank" href="/middleware/location">location</a>
{{% /details %}}

{{% details title="Redirects" closed="true" %}}
* Simple redirect functionality (all of `permanent`, `temporary`, `found`, `moved_permanently`)
* HTTP to HTTPs Redirects
* Dynamic URL Rewriting
* See <a target="_blank" href="/middleware/redirect">redirect</a>
{{% /details %}}

{{% details title="Reverse Proxy" closed="true" %}}
* Seamlessly Proxy to downstream HTTP services
* Hostname and SNI overrides
* Header overrides
* Multiple backends per host, with load balancing and failover support
* Automatic retries for Idempotent requests
* Configurable error pages
* See <a target="_blank" href="/middleware/proxy">proxy</a>
{{% /details %}}

{{% details title="Range Requests" closed="true" %}}
* Partial content delivery support, so clients can resume downloads or stream large files efficiently.
* See <a target="_blank" href="/middleware/static_assets">static_assets</a>
{{% /details %}}

{{% details title="Static File Server" closed="true" %}}
Efficiently serves static assets with proper content types and caching headers.
* Auto index generation
* Configurable in-memory caching for faster serving of small and frequently accessed files.
* Auto try `.html` extension (for cleaner paths)
* Configurable fallback behaviour (e.g. route request misses to an index.html for SPAs)
* See <a target="_blank" href="/middleware/static_assets">static_assets</a>
{{% /details %}}

{{% details title="Multiple Binds" closed="true" %}}
* Itsi can listen on multiple IP addresses or ports simultaneously for flexible deployment.
* Unix socket binds (both plain-text and TLS) are supported.
* See <a target="_blank" href="/options/bind">bind</a>.
{{% /details %}}

## DevOps
{{% details title="File Watcher & Live Reloading" closed="true" %}}
* Monitors file changes and to automatically reloads configuration or content.
* Use custom watchers to e.g. trigger frontend builds on file changes.
* See <a target="_blank" href="/getting_started/local_development">local_development</a>, <a target="_blank" href="/options/auto_reload_config">auto_reload_config</a>, and <a target="_blank" href="/options/watch">watch</a>.
{{% /details %}}

{{% details title="LSP and shell completion support" closed="true" %}}
* The bundled RubyLSP addon provides rich inline documentation and hover support when editing `Itsi.rb` files.
* Shell completion support (just add `eval "$(itsi --install-completions)"` to the bottom of your shell init file)
* See <a target="_blank" href="/getting_started/local_development">local_development</a>.
{{% /details %}}

{{% details title="Status Reporting" closed="true" %}}
* Send SIGUSR2 to trigger detailed status report across all Itsi processes.
* See <a target="_blank" href="/getting_started/signals">signals</a>.
{{% /details %}}

{{% details title="Granular Logging" closed="true" %}}
* Support logging using plain-text and structured `JSON` formats.
* Support `STDOUT`, file-system and combined log sinks.
* Apply selective log levels for specific log targets only.
* Configurable request logging middleware, with custom log templates.
* See <a target="_blank" href="/middleware/log_requests">Request Logs</a> & <a target="_blank" href="/getting_started/logging">Logging</a>.
{{% /details %}}

{{% details title="Hot Config Reloads & Config File Validation" closed="true" %}}
* Zero-downtime config file reloads.
* Phased restart support when running in `cluster` mode
* Config file testing and dry-run functionality
* See <a target="_blank" href="/getting_started/signals">signals</a> and <a target="_blank" href="/options/auto_reload_config">auto_reload_config</a>.
{{% /details %}}

{{% details title="Configurable Error Responses" closed="true" %}}
* Provide your own exception responses (HTML and JSON) for all common exception scenarios, or simply rely on the light-weight defaults.
* See <a target="_blank" href="/middleware/error_response">error_responses</a>.
{{% /details %}}

{{% details title="Management Signals" closed="true" %}}
* Use a full suite of Unix signals to control your live Itsi cluster.
* Add or remove workers on the fly, reload config, generate status reports etc.
* See <a target="_blank" href="/getting_started/signals">signals</a>.
{{% /details %}}

## Security
{{% details title="JWT/API Key/Basic Auth" closed="true" %}}
* Apply common authentication patterns at the middleware layer.
* API Key (`bcrypt`, `argon2`, `sha256`, `sha512`)
* JWT (`hs256`, `HS384`, `HS512`, `RS256`, `RS384`, `RS512`, `ES256`, `ES384`, `PS256`, `PS384`, `PS512`)
* Basic Auth (`bcrypt`, `argon2`, `sha256`, `sha512`)

Itsi also comes bundled with a passfile generator, to help you manage your password hashes effectively.

* See <a target="_blank" href="/middleware/auth_jwt">auth_jwt</a>, <a target="_blank" href="/middleware/auth_api_key">auth_api_key</a>, <a target="_blank" href="/middleware/auth_basic">auth_basic</a> and <a target="_blank" href="/utilities/passfile_generator">passfile</a>.
{{% /details %}}

{{% details title="Automatic Let's Encrypt Certificates" closed="true" %}}
* Automated provisioning of Let's Encrypt certificates.
* File system caching of certificate data to avoid excessive API calls.
* Supports usage of subject alternative names (SANs) for certificates that span multiple domains/sub-domains.
* See <a target="_blank" href="/options/certificates#production-certificates-lets-encrypt">certificates</a>.
{{% /details %}}

{{% details title="Automatic Development Certificates" closed="true" %}}
* Easily mirror your production SSL set-up in Development
* Custom local CA generation (add this CA cert to your trusted root certificates for warning-less SSL during local development)
* See <a target="_blank" href="/options/certificates#development">certificates</a>.
{{% /details %}}

{{% details title="(Distributed) Rate Limiting" closed="true" %}}
* Combine any number of configurable rate limits
* Support for a `Redis` backend for distributed rate limiting (falls back to in-memory backend)
* In-memory backend for simple setups and local development.
* See <a target="_blank" href="/middleware/rate_limit">rate_limit</a>.
{{% /details %}}

{{% details title="Allow & Deny Lists" closed="true" %}}
* IP Allow lists to limit access to a specific set of IP addresses or blocks.
* IP Deny lists to block access from specific IP addresses or blocks.
* See <a target="_blank" href="/middleware/allow_list">allow_list</a> & <a target="_blank" href="/middleware/deny_list">deny_list</a>.
{{% /details %}}

{{% details title="Intrusion Protection" closed="true" %}}
* Automatically scan request paths and headers for known malicious patterns
* Configurable ban rules to block offenders for a specified duration.
* See <a target="_blank" href="/middleware/intrusion_protection">intrusion_protection</a>
{{% /details %}}

{{% details title="Slowhttp attack prevention" closed="true" %}}
* Protections against several slowhttp attacks (e.g. Slowloris, Slowbody), through header and request timeouts and maximum request body sizes.
* See <a target="_blank" href="/options/max_body">max_body</a>, <a target="_blank" href="/options/request_timeout">request_timeout</a> and <a target="_blank" href="/options/header_read_timeout">header_read_timeout</a>
{{% /details %}}

{{% details title="CSP Reporting" closed="true" %}}
* Simple configuration for enabling CSP headers.
* Support for hosting a CSP reporting endpoint to track violations of CSPs running in reporting only mode.
* See <a target="_blank" href="/middleware/csp">CSP</a>
{{% /details %}}

## Protocols & Standards
{{% details title="HTTP2" closed="true" %}}
* Benefit from connection multiplexing by using http2 all the way from client to app/file server.
* `Itsi`'s  underlying HTTP1 and 2 implementations are provided directly by <a target="_blank"  href="https://github.com/hyperium/hyper">hyper</a>. Itsi simply exposes these existing capabilities. This means that once <a target="_blank" href="https://hyper.rs/contrib/roadmap/#http3">h3</a> lands in Hyper - we'll get it in Itsi too!
{{% /details %}}

{{% details title="Rack Server" closed="true" %}}
* Rack compliant. Itsi plays nicely with your existing Rack-based applications and middleware.
* See <a target="_blank" href="/middleware/run">run</a> and <a target="_blank" href="/middleware/rackup_file">rackup_file</a>
{{% /details %}}

{{% details title="gRPC Server" closed="true" %}}
* Itsi is compatible with ruby `grpc` service handlers and can
replace the <a  target="_blank" href="https://github.com/grpc/grpc/blob/master/src/ruby/README.md">official</a> Ruby gRPC server implementation for a free performance boost!
* Consider enabling [non-blocking IO](/options/fiber_scheduler) to further enhance performance.
* Support for gRPC server reflection (use with tools like evans and Postman for easy service discovery)
* Support for gzip and zlib compression
* See <a target="_blank" href="/middleware/grpc">grpc</a>
{{% /details %}}

{{% details title="gRPC+REST compatibility mode" closed="true" %}}
* Itsi provides a `gRPC+REST` compatibility layer for easy reuse of gRPC endpoints by clients and environments that are not gRPC capable. Invoke unidirectional and streaming endpoints using plain-old JSON.
* See <a target="_blank" href="/middleware/grpc">grpc</a>
{{< callout type="warn" >}}
Note: This is not the same as <a target="_blank" href="https://grpc.io/blog/grpc-with-json/">gRPC with JSON</a> which swaps out protobuf for JSON but still relies on gRPC's underlying framing mechanics.
{{< /callout >}}
{{% /details %}}

{{% details title="WebSockets" closed="true" %}}
* WebSocket support for Rack apps (e.g. <a target="_blank" a href="https://guides.rubyonrails.org/action_cable_overview.html">ActionCable</a>)
{{% /details %}}

## Concurrency & Performance
{{% details title="Cluster Mode" closed="true" %}}
* Supports running in a clustered mode, to fully leverage multi-core systems.
* See <a target="_blank" href="/options/workers">workers</a>.
{{% /details %}}

{{% details title="Non-blocking(Fiber Scheduler) Mode" closed="true" %}}
* Support for Ruby’s fiber scheduler for non-blocking concurrency, boosting performance during I/O operations.
* Use Itsi's own high-performance built-in <a target="_blank" href="/itsi_scheduler">Fiber Scheduler</a> or if your prefer you can bring your own!
* See <a target="_blank" href="/options/fiber_scheduler">fiber_scheduler</a>.
{{% /details %}}

{{% details title="Hybrid Blocking/Non-Blocking Mode" closed="true" %}}
* `Itsi` allows you to split endpoints between using a Fiber scheduler versus running using the traditional blocking IO model. This allows you to dip your toes into the waters of Ruby's new non-blocking IO, without having to port an entire application at once!
* See <a target="_blank" href="/options/scheduler_threads">scheduler_threads</a>.
{{% /details %}}

{{% details title="Non-blocking by design" closed="true" %}}
* Itsi is underpinned by <a target="_blank" href="https://hyper.rs/">hyper</a> and <a target="_blank" href="https://tokio.rs/">tokio</a> and as such is fundamentally an evented, non-blocking server. Whether you're proxying, serving large files, or delegating to Ruby endpoints, `Itsi` remains responsive, even under heavy load.
{{% /details %}}

## Ruby
{{% details title="Preloading" closed="true" %}}
* Preload your Ruby application code before forking to benefit from reduced memory through CoW
* Alternatively, use groups in bundler to target specific gems or dependencies for preloading
* See <a target="_blank" href="/options/preload">preload</a>.
{{% /details %}}

{{% details title="Streaming Response Bodies" closed="true" %}}
* For both <a target="_blank" href="https://github.com/rack/rack/blob/main/SPEC.rdoc#the-body-">streaming</a> and <a target="_blank" href="https://github.com/rack/rack/blob/main/SPEC.rdoc#enumerable-body-">Enumerable</a> bodies, Itsi sends data to the client as soon as it is available. This means modules like <a target="_blank"  href="https://api.rubyonrails.org/v7.1/classes/ActionController/Live.html">ActionController</a> behave as expected, and the minimal buffering keeps `Itsi`'s memory footprint consistently low.
{{% /details %}}

{{% details title="Streaming Request Bodies" closed="true" %}}
* Itsi supports streaming incoming request bodies too, for efficient processing of large simultaneous input streams (Disabled by default for maximum compatibility).
* See <a target="_blank" href="/options/stream_body">stream_body</a>.
{{% /details %}}

{{% details title="Full & Partial Rack Hijacking" closed="true" %}}
* Itsi supports both <a href="https://github.com/rack/rack/blob/main/SPEC.rdoc#hijacking-" target="_blank">full</a> and partial Rack hijacking. Even over HTTP2!
{{< callout type="warn" >}}
By design, Full hijacking assumes you are writing a raw HTTP1 response directly to a raw connection stream.
Itsi's support for full hijack over HTTP2 is similar to what you would see if running a dedicated reverse proxy in front of a Ruby app.
Itsi translates that request from HTTP1 to HTTP2, in real-time, allowing full hijacking endpoints to write HTTP1 and the client to receive HTTP2.
{{< /callout >}}
{{% /details %}}

{{% details title="Sendfile" closed="true" %}}
* Itsi allows Ruby apps to set a `X-Sendfile` header to enable efficient, streaming file transfers, outside of Ruby, via fast native code.
* See <a target="_blank" href="/middleware/run">run</a> and <a target="_blank" href="/middleware/rackup_file">rackup_file</a>.

{{< callout type="info" >}}
Note that despite the header being named `X-Sendfile`, Itsi does not use the Sendfile system call, instead delegating the efficient streaming to Tokio's native asynchronous file streaming capabilities.
{{< /callout >}}

{{% /details %}}

{{% details title="Graceful Memory Limits" closed="true" %}}
* Itsi allows you to specify memory limits for Ruby processes. When the limit is reached, Itsi gracefully terminates the process and also invokes a dedicated `after_memory_threshold_reached` callback,
so that you can log the event for further analysis.
* See <a target="_blank" href="/options/worker_memory_limit">worker_memory_limit</a> and  <a target="_blank" href="/options/after_memory_threshold_reached">after_memory_threshold_reached</a>.
{{% /details %}}

{{% details title="OOB GC" closed="true" %}}
* Itsi can be configured to periodically trigger GC every N idle periods (where an idle period is defined as a time where no requests are currently queued).
* Periodic triggering of GC outside of the request flow can help reduce the impact of GC on latency.
* See <a target="_blank" href="/options/oob_gc">oob_gc_threshold</a>
{{% /details %}}

{{% details title="'Rackless' Ruby Apps" closed="true" %}}
* Itsi allows definition of ultra-light-weight Ruby web-apps, using plain old functions and procs.
* For simple endpoints this barebones option can provide a substantial increase in throughput over a Rack request (primarily by avoiding allocating the env hash and response array)
* See <a target="_blank" href="/middleware/endpoint">endpoint</a>
{{% /details %}}
