reuse_port true
reuse_address true

ruby_thread_request_backlog_size 10000

run(
  ->(env){
    [200, {"test" => "this"}, ->(stream){
      stream.send_and_close "Ok"
    }]
  }
)


# oob_gc_responses_threshold 5 # Trigger GC every N gaps in the request queue.
