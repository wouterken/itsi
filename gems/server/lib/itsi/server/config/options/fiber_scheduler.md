---
title: Fiber Scheduler
url: /options/fiber_scheduler
---
Itsi supports processing requests in threads that are managed by a fiber scheduler.
This allows Itsi to process a very large number of IO heavy requests concurrently without the memory and context switching overhead of managing multiple threads.

Enabling Fiber Scheduler mode can drastically improve application performance if you perform large amounts of blocking IO operations.


## Configuration File
```ruby {filename="Itsi.rb"}
# Enable Itsi's fiber scheduler mode
# (This will use an instance of `Itsi::Scheduler`
# This is Itsi's built in Fiber scheduler.)
fiber_scheduler true
```

```ruby {filename="Itsi.rb"}
# In the spirit of the Fiber::Scheduler interface,
# you can bring your own scheduler!.
# E.g. using the scheduler from the popular Async library.
fiber_scheduler "Async::Scheduler"
```

{{< callout type="warning" >}}
Running in Fiber scheduler mode can be a huge performance boon, but it's not without tradeoffs. Because it enables drastically more in-flight requests,
it can have a substantial impact on memory usage. Similarly, it can increase the amount of simultaneous demand on pooled resources (like database connections or network sockets)
and can cause increased contention on shared locks.

While well-tuned Fiber based servers can drastically outperform their blocking counterparts in some scenarios, the above compromises can make it an unsafe blanket choice, particularly for some large applications with dependencies not specifically designed for a cooperative multitasking environment.

To see Itsi's recommended approach to enjoying the benefits of a Fiber scheduler while managing these risks, consider using Itsi's [hybrid mode](/options/scheduler_threads).
{{< /callout >}}
