---
title: "Benchmarks"
description: "Performance benchmarks for Itsi across different devices and configurations"
type: docs
weight: 100
toc: false
sidebar:
  exclude: true
---

<br/>
<div id="root" style="min-height: 550px;">
</div>

<style>
.hextra-toc{
  width: 0 !important;
}

html{
  scrollbar-gutter: stable;
}
</style>


<link rel="stylesheet" href="/styles/benchmark-dashboard.css"/>

<script>
window.addEventListener("load", () => {
  const script = document.createElement("script");
  script.type = "text/javascript";
  script.src="/scripts/benchmark-dashboard.iife.js";
  document.body.appendChild(script);
})
</script>

## Intro
This benchmark suite tests a variety of the most widely used Rack servers, file servers, reverse proxies, and Ruby gRPC servers across diverse workloads on CPUs that vary significantly in their capabilities. Each benchmark includes error rates and p95 request latency.
* AMD Ryzen 5600 (6 Core / 12 Thread). AMD64
* Apple M1 Pro (6P cores + 2E cores). ARM64
* Intel N97 (4 Cores). AMD64

All tests were run on Ruby 3.4.3 with YJIT enabled.


### Disclaimer
All source code for these benchmarks is accessible [for review and reproduction](https://github.com/wouterken/itsi-server-benchmarks).

While the intention is to fairly represent all software being tested, some results may not accurately reflect the best performance due to non-optimal configuration or errors in the test methodology. I welcome PRs to correct any such misconfiguration.

Before using performance to rationalize a server switch, always measure first and confirm that server performance is actually a bottleneck for your workload.

The following known caveats apply to these results:
* **No isolated testing environment**: Benchmarks were run on personal development machines without measures to isolate CPU, memory, or disk I/O from background processes. This may introduce variability or skew in some results.
* **Test clients and server were run on the same device**: Running both the load generator and the server on the same host introduces contention for CPU and memory resources. All network traffic occurs over the loopback interface, which does not reflect real-world network latency or congestion.
* **No cooldown between concurrency sweeps**:  Concurrency tiers were tested sequentially on a single persistent server process, without a cooldown period between runs. Servers overloaded at high concurrency levels may exhibit degraded performance in subsequent tiers due to lingering backlog or thread exhaustion.
* **Short test duration**: Each benchmark ran for only 3 seconds. This favors servers that perform well under cold start or burst conditions and may disadvantage those that rely on longer warm-up phases. This tradeoff was made intentionally to reduce the total benchmarking time and device occupation when generating large result sets.


## Summary
### Rack - Fast Handlers + Small Response Bodies
These tests demonstrate the largest observed difference between pure-Ruby servers and native alternatives, because lightweight endpoints mean native servers spend almost all their time in optimized native code. However, this is rarely the case in true production workloads and is not a realistic indication of the performance impact you might see when switching to a more dynamic workload.
Examples:
* [Empty Response](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=empty_response&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)
* [Hello World](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=hello_world&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)

### Rack - Non-blocking
Servers that support non-blocking IO (e.g., Falcon, Itsi) provide the best throughput for test cases that contain blocking operations, as they allow many more simultaneous requests to be multiplexed across fewer threads.
As expected, performance differences narrow as we increase thread counts and workers for blocking servers.
* [Chunked](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=chunked&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)
* [IO heavy](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=io_heavy&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)
* [Non Blocking Big Delay](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=nonblocking_big_delay&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)
* [Non Blocking Small Delay](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=nonblocking_many_small_delay&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)

### Rack - Heavy CPU
The above differences all but disappear on CPU-heavy workloads, as most time is spent bottlenecked in CPU-intensive code that behaves identically in all tests regardless of server implementation.
* [CPU Heavy](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=cpu_heavy&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)

### Rack - Large Response Bodies
When dealing with large bodies, much less time is spent in server code or Ruby handler code, and most performance impact results from how efficiently the server can flush bytes onto the network. Here, techniques like buffering and using vectored operations can reflect a throughput advantage.
* [Response Size Large](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=response_size_large&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)

### Mixed Workload (Static + Dynamic)
A mixed workload test, requesting an image, a static HTML file, and a dynamic endpoint simultaneously aiming to represent more realistic and varied production workloads. This benchmark demonstrates the advantage of the typical production Ruby app server deployment, fronted by a reverse proxy, as an efficient alternative to serving static assets directly from an app server, where static asset requests are subject to head-of-line blocking. This test also compares this traditional setup to an alternative: using an all-in-one solution like Itsi, which serves static assets from an separate asynchronous reactor, isolated from Ruby worker threads to provide equivalent or better performance, without needing to spin up a dedicated proxy process.
* [Static Dynamic Mixed](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=static_dynamic_mixed&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)

### File Serving
Native servers and reverse proxies offer the best file-serving performance as they can do this at incredible speeds without contending for the GVL.
* [Static Large](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=static_large&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)
* [Static Small](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=static_small&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)

### gRPC
The server bundled with the Ruby [gRPC gem](https://github.com/grpc/grpc/tree/master/src/ruby) is the de facto standard for gRPC services in the Ruby ecosystem. However, it doesn't utilize a Fiber scheduler and therefore is more prone to thread exhaustion errors, particularly for streaming calls, than Itsi.
* [Echo Stream](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=echo_stream&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)
* [Process Payment](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=process_payment&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)
* [Echo Collect](?cpu=amd_ryzen_5_5600x_6_core_processor&testCase=echo_collect&threads=1&workers=1&concurrency=10&http2=all&xAxis=concurrency&metric=rps&visibleServers=agoo%2Cfalcon%2Citsi%2Cpuma%2Cpuma__caddy%2Cpuma__h2o%2Cpuma__itsi%2Cpuma__nginx%2Cpuma__thrust%2Cunicorn%2Ciodine%2Ccaddy%2Ch2o%2Cnginx%2Cgrpc_server.rb)
