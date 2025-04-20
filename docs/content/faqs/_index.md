---
title: F.A.Qs
type: docs
prev: middleware/
next: utilities/
---

{{% details title="Is it just for Ruby applications?" closed="true" %}}
**No!** While one of Itsi's goals is to be the most frictionless way to get Ruby onto the web, it stands alone as a powerful Reverse Proxy, Static File Server and API Gateway.

You can have Itsi sit in front of *any* application that speaks HTTP and immediately benefit from security middleware, performance enhancements, and more.
You will need to write a little bit of Ruby, just to configure your Itsi server inside the `Itsi.rb` file. Who knows, maybe you'll learn to love it!
{{% /details %}}

{{% details title="What's it written in?" closed="true" %}}
The heart of Itsi is a Rust server, leaning *heavily* on [tokio](https://tokio.rs) and [hyper](https://hyper.rs) and many other fantastic and high performance Rust libraries.
Take a look at the [Cargo.toml](https://github.com/wouterken/itsi/blob/main/crates/itsi_server/Cargo.toml) to see them all.

This is exposed via a robust and ergonomic Ruby DSL.
{{% /details %}}

{{% details title="What License is it under?" closed="true" %}}
Itsi is an open source project licensed under the terms of the [LGPLv3](https://www.gnu.org/licenses/lgpl-3.0.en.html).

You can integrate and use Itsi in your projects—whether they are open source or proprietary—without any licensing fees or obligations, as long as you use Itsi in its unmodified form. However, if you modify Itsi’s source code and distribute the modified version, you are required to release your modifications under the same LGPLv3 license.

If these terms do not meet your project’s needs or if you require bespoke support and legal assurances, Itsi is also available under alternative commercial licensing options.
Please contact commercial@itsi.fyi for more information.
{{% /details %}}
