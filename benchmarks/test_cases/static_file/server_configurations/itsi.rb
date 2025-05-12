# First attempt to serve incoming requests as static assets,
# falling through to our rack-mapp on not-found.
static_assets root_dir: './apps', not_found_behavior: 'fallthrough'

# To make benchmarks fair. If we use too small a default, Itsi will start applying backpressure
# by returning 503s under heavy loads for distant queued requests to Ruby,
# causing an artificial increase in throughput
ruby_thread_request_backlog_size 100_000
