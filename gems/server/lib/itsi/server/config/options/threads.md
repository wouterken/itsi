---
title: Threads
url: /options/threads
weight: 2
---
Itsi supports running in threaded mode. Threaded mode is helpful for applications that require high concurrency and low latency.
Note, that while threading allows for concurrent execution of tasks, it also introduces additional overhead due to context switching between threads. This overhead can be significant, especially when dealing with a large number of concurrent tasks. To minimize this overhead, it's recommended to keep the number of threads low, and tune this parameter based on your specific use case.

When running Itsi in blocking mode, the total number of concurrent requests will be equal to `workers` x `threads`

## Configuration File
The number of threads to use can be specified inside the configuration file (usually `./Itsi.rb` at the project root)
using the `threads` function.

## Examples
```ruby {filename="Itsi.rb"}
# Starts each worker with a single thread
threads 1
```

```ruby {filename="Itsi.rb"}
# Each worker will start 4 threads
threads 4
```

Threads increase concurrency at the expense of memory usage and CPU overhead.
For IO heavy workloads, consider using non-blocking mode instead, which can achieve higher concurrency with fewer threads.
Non-blocking mode and threads can be used simultaneously in a hybrid configuration.

## Command Line
You can also override the number of threads using either the `-t` or `--threads` command line option.
E.g.

```bash
itsi -t 3
```
