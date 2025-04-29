## Hybrid Scheduler Mode Example
This example shows how you can route some requests to use traditional blocking threads,
and others to use threads that have a Fiber scheduler enabled.

First, run the slow_service inside ./slow_service.
E.g.
```bash
cd ./slow_service
itsi
```

Then in a second tab, run the hybrid_scheduler_mode example.
E.g.
```bash
itsi
```

If you run a benchmark on the `/nonblocking` endpoint, you
should see Itsi will concurrent execute all requests. By contrast, running on
the `/blocking` endpoint should show drastically worse throughput due to requests running sequentially.

```bash
 # Non-blocking mode. 100 requests at a time, service takes 2s to respond. Throughput of ~50rps expected.
  ❯ wrk  http://0.0.0.0:3000/nonblocking -c 100 -d 10
 Running 10s test @ http://0.0.0.0:3000/nonblocking
   2 threads and 100 connections
   Thread Stats   Avg      Stdev     Max   +/- Stdev
     Latency     0.00us    0.00us   0.00us     nan%
     Req/Sec    65.00    109.41   316.00     84.62%
   500 requests in 10.10s, 69.34KB read
   Socket errors: connect 0, read 0, write 0, timeout 500
 Requests/sec:     49.50
 Transfer/sec:      6.86KB
 # Blocking mode. Requests are executed sequentially , service takes 2s to respond. Throughput of ~0.5rps expected.
  ❯ wrk  http://0.0.0.0:3000/nonblocking -c 100 -d 10
 Running 10s test @ http://0.0.0.0:3000/blocking
   2 threads and 100 connections
   Thread Stats   Avg      Stdev     Max   +/- Stdev
     Latency     0.00us    0.00us   0.00us     nan%
     Req/Sec     0.00      0.00     0.00    100.00%
   5 requests in 10.10s, 710.00B read
   Socket errors: connect 0, read 0, write 0, timeout 5
 Requests/sec:      0.49
```

See the `Itsi.rb` file for more details.
