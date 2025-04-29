# frozen_string_literal: true

# Before running this test, go to the ./slow_service directory, and start the slow running service
# on port 3005, by running `itsi`.

# Single thread for blocking test.
threads 1

# Single thread for non-blocking test.
scheduler_threads 1

shutdown_timeout 1.0 # Shutdown Timeout in Seconds

# Small backlogs for test purposes
ruby_thread_request_backlog_size 100

# Enable Itsi's fiber scheduler.
fiber_scheduler true

# Send requests to http://localhost:3000/nonblocking.
# The app at config.ru runs a slow endpoint (takes 3+ seconds).
# When using this endpoint you can have a very large number of simultaneous inflight requests.
#
#
# Example benchmark:
# Non-blocking mode. 100 requests at a time, service takes 2s to respond. Throughput of ~50rps expected.
#  ❯ wrk  http://0.0.0.0:3000/nonblocking -c 100 -d 10
# Running 10s test @ http://0.0.0.0:3000/nonblocking
#   2 threads and 100 connections
#   Thread Stats   Avg      Stdev     Max   +/- Stdev
#     Latency     0.00us    0.00us   0.00us     nan%
#     Req/Sec    65.00    109.41   316.00     84.62%
#   500 requests in 10.10s, 69.34KB read
#   Socket errors: connect 0, read 0, write 0, timeout 500
# Requests/sec:     49.50
# Transfer/sec:      6.86KB

location 'nonblocking' do
  rackup_file 'config.ru', nonblocking: true, script_name: ''
end


# Send requests to http://localhost:3000/blocking.
# The app at config.ru runs a slow endpoint (takes 3+ seconds).
# When using this endpoint, concurrency is limited by the number of blocking threads available.
#
# Blocking mode. Requests are executed sequentially , service takes 2s to respond. Throughput of ~0.5rps expected.
#  ❯ wrk  http://0.0.0.0:3000/nonblocking -c 100 -d 10
# Running 10s test @ http://0.0.0.0:3000/blocking
#   2 threads and 100 connections
#   Thread Stats   Avg      Stdev     Max   +/- Stdev
#     Latency     0.00us    0.00us   0.00us     nan%
#     Req/Sec     0.00      0.00     0.00    100.00%
#   5 requests in 10.10s, 710.00B read
#   Socket errors: connect 0, read 0, write 0, timeout 5
# Requests/sec:      0.49

location 'blocking' do
  rackup_file 'config.ru', script_name: ''
end
