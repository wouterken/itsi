---
title: Options
type: docs
next: /options/server/lib/itsi/server/config/options/workers/
url: /options
cascade:
  type: docs
weight: 1
---

Most of Itsi's capabilities are unlocked via the Itsi.rb config file.
The config file uses a simple DSL, where you can write plain Ruby to define your application's configuration.
For the best development experience, be sure to use [RubyLSP](https://shopify.github.io/ruby-lsp/) for snippets, autocomplete and documentation, right in your editor.

{{< details title="An example Itsi.rb file" >}}


```ruby {filename="Itsi.rb"}
workers 2
threads 2
scheduler_threads 3

fiber_scheduler true

rate_limiter requests: 100, seconds: 10

auth_basic realm: "Restricted Area", credentials_file: "credentials.txt"

location "/app" do
  get "/" do |req|
    req.ok "Hello, World!"
  end
end

```
{{< /details >}}
