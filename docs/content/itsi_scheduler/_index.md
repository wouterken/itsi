---
title: Itsi Scheduler
type: docs
weight: 4
sidebar:
  exclude: true
---
<img src="itsi-scheduler-100.png"  width="80px" style="display: block; margin-left: auto; margin-right: auto;">

`Itsi Scheduler` is an implementation of a Ruby [Fiber Scheduler](https://docs.ruby-lang.org/en/3.2/Fiber/Scheduler.html).

When combined with Itsi server, you can write endpoints that look and feel exactly like regular synchronous Ruby code,
but behind the scenes, the scheduler will transparently yield and resume concurrent request fibers, to prevent threads from blocking and greatly increase concurrency in IO heavy workloads.

If you're purely after a light-weight, yet efficient Ruby scheduler,
you can use Itsi Scheduler as a standalone scheduler for any Ruby application.

Just use `Fiber.set_scheduler` to set an instance `Itsi::Scheduler` as a scheduler to opt in to this IO weaving behaviour
*automatically* for all blocking IO.

### Primer on Fiber Schedulers

Fiber schedulers are a way to automatically manage the execution of non-blocking fibers in Ruby. A scheduler is responsible for the automatic pausing and resumption of Fibers based
on whether or not they are awaiting IO operations.
Ruby's Fiber scheduler implementation automatically invokes the current Fiber scheduler (if it exists) for each blocking operation, allowing it to seamlessly drive the execution of huge numbers of simultaneous non-blocking fibers
while ensuring the main thread is never blocked on IO.

This behind the scenes magic allows Ruby to provide async IO (just like we find in languages with `async/await` like `Rust`, `C#`, `JavaScript`) *but* with the added beauty
that synchronous and asynchronous code is identical! (I.e. Ruby's functions are [colorless](https://journal.stuffwithstuff.com/2015/02/01/what-color-is-your-function/))

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
  {{< tab >}}**Windows**: Itsi currently doesn't support native Windows builds, but it runs great on [https://learn.microsoft.com/en-us/windows/wsl/install](WSL).

  Follow the linked instructions to Install a linux distribution like Ubuntu or Debian and then follow the instructions in the Linux tab.
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

and run many blocking operations simultaneously all while occupying only a single Ruby thread!
{{% /steps %}}
