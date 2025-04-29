---
title: Scheduler Threads
url: /options/scheduler_threads
---

You can explicitly spawn a pool of non-blocking scheduler threads and divide work across traditional/blocking and non-blocking threads, using [location](/middleware/location) blocks.

This allows you to safely dip your toes into using non-blocking threads for specific I/O heavy operations without having to port an entire application to non-blocking I/O.

To use blocking and non-blocking threads in conjunction you need to perform several steps.
1. Configure a thread pool using [threads](/middleware/threads)
2. Configure a **separate** non-blocking thread pool using [scheduler_threads](/middleware/scheduler_threads) (By opting into this option you automatically make the ordinary thread pool blocking)
3. Enable a [fiber_scheduler](/options/fiber_scheduler)
4. Mount your app as non-blocking for selected routes. (Using either [run](/middleware/run), [rackup_file](/middleware/rackup_file) or [endpoint](/middleware/endpoint))

## Configuration
Here is an example configuration of the all of the above E.g.

```ruby {filename=Itsi.rb}

threads 3 # 3 threads (opting into scheduler threads make these blocking)
scheduler_threads 1 # 1 non-blocking scheduler thread
fiber_scheduler true

# We mount the same app *twice*.
# For a specific route prefix, all requests will be sent to non blocking threads.
# All others fall through to the default mount

location "/heavy_io/*" do
  # You can optionally use the `script_name: ""` option here to set the base path for the mounted app (useful if a nested app
  # should still serve requests as if it was mounted at the root).
  # Otherwise it will infer the script-name based on the parent location block.
  rackup_file "./config.ru", nonblocking: true
end

rackup_file "./config.ru"

```
## Examples.

See [https://github.com/wouterken/itsi/tree/main/examples/hybrid_scheduler_mode](hybrid_scheduler_mode) example in the Git repository.
