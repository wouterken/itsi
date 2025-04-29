---
title: Itsi Scheduler
type: docs
weight: 4
sidebar:
  exclude: true
prev: utilities/
next: acknowledgements/
---
<img src="itsi-scheduler-100.png"  width="80px" style="display: block; margin-left: auto; margin-right: auto;">

`Itsi Scheduler` is an implementation of a Ruby [Fiber Scheduler](https://docs.ruby-lang.org/en/3.2/Fiber/Scheduler.html).

When combined with Itsi Server, you can write endpoints that look just like regular synchronous Ruby code. Behind the scenes, the scheduler will transparently pause and resume fibers to prevent threads from blocking, greatly increasing throughput for I/O-heavy workloads

If you're purely after a lightweight, yet efficient Ruby scheduler,
you can use Itsi Scheduler as a standalone scheduler for any Ruby application.

Just use `Fiber.set_scheduler` to set an instance `Itsi::Scheduler` as a scheduler to opt in to this IO weaving behavior
*automatically* for all blocking IO.

### Primer on Fiber Schedulers

Fiber schedulers are a way to automatically manage the execution of non-blocking fibers in Ruby. A scheduler is responsible for the automatic pausing and resumption of Fibers based
on whether or not they are awaiting IO operations.
Ruby's Fiber scheduler implementation automatically invokes the current Fiber scheduler (if it exists) for each blocking operation, allowing it to seamlessly drive the execution of huge numbers of simultaneous non-blocking fibers
while ensuring the main thread is never blocked on IO.

This behind the scenes magic allows Ruby to provide async IO (just like we find in languages with `async/await` like `Rust`, `C#`, `JavaScript`) *but* with the added beauty
that synchronous and asynchronous code is identical! (i.e. Ruby's functions are [colorless](https://journal.stuffwithstuff.com/2015/02/01/what-color-is-your-function/))

## Getting Started
To install and use Itsi Scheduler follow the below instructions:

{{% steps %}}


### 1 - Install Itsi Scheduler

{{< tabs items="Linux,Mac,Windows" >}}
  {{< tab >}}
  **Prerequisites**

  You'll need at least `build-essential` and `libclang-dev` installed to build Itsi on Linux.
  E.g.
  ```bash
  apt-get install build-essential libclang-dev
  ```

  Then use `gem` to install the Itsi package. This will in turn install both the
  `itsi-server` gem, and the `itsi-scheduler` gem.


  ```bash
  gem install itsi-scheduler
  ```

  {{< callout type="info" >}}
  Are you looking for Itsi server too? In this case, use `gem install itsi` (to get both Itsi scheduler and server)
  Or `gem install itsi-server` to install just Itsi Server.
  {{< /callout >}}

  {{< /tab >}}
  {{< tab >}}
  **Mac**:
  ```bash
  gem install itsi-scheduler
  ```
  {{< callout type="info" >}}
  Are you looking for Itsi server too? In this case, use `gem install itsi` (to get both Itsi scheduler and server)
  Or `gem install itsi-server` to install just Itsi Server.
  {{< /callout >}}

  {{< /tab >}}
  {{< tab >}}**Windows**: Itsi currently doesn't support native Windows builds, but it runs well on [https://learn.microsoft.com/en-us/windows/wsl/install](WSL).

  Follow the linked instructions to Install a Linux distribution like Ubuntu or Debian and then follow the instructions in the Linux tab.
  {{< /tab >}}

{{< /tabs >}}

### 2 - Use Itsi Scheduler

Great! You now have Itsi Scheduler installed.
Now you can run code like this:

```ruby
require 'itsi/scheduler'
require 'socket'
results = Thread.new do
  Fiber.set_scheduler Itsi::Scheduler.new
  results = []
  Fiber.schedule do
    results << Addrinfo.getaddrinfo("www.ruby-lang.org", 80, nil, :STREAM)
  end
  Fiber.schedule do
    results << Addrinfo.getaddrinfo("www.google.com", 80, nil, :STREAM)
  end
  results
end.value

puts results.map(&:inspect)
```
to run many blocking operations simultaneously all while occupying only a single Ruby thread!

### 3 (Optional) - Enable Scheduler Refinements
You can opt-in to a tiny set of Ruby refinements provided by the `Itsi::Scheduler` to make usage even more ergonomic.
By opting in to this refinement (using `using Itsi::Scheduler`) you gain access to the top-level #schedule(&block) method, as well
as enumerable methods #schedule_each, and #schedule_map.

```ruby

using Itsi::Scheduler

# Fire-and-forget: 100 HTTP calls in parallel
100.times.schedule_each do |i|
  Net::HTTP.get(URI("https://example.com/#{i}"))
end

# Concurrent transform that keeps the original order
squares = (1..20).schedule_map { |n| n * n }
puts squares.inspect
# => [1, 4, 9, 16, … 400]

# Manual orchestration — still one thread
schedule do
  a, b = Queue.new, Queue.new

  schedule { a << Net::HTTP.get(URI("https://httpbin.org/get")) }
  schedule { b << Net::HTTP.get(URI("https://httpbin.org/uuid")) }

  puts a.pop
  puts b.pop
end
```
{{% /steps %}}
