---
title: Options
type: docs
next: auto_reload_config/
url: /options
prev: configuration/
cascade:
  type: docs
weight: 1
---

Most of Itsi's capabilities are unlocked via the Itsi.rb config file.
The config file uses a simple DSL, where you can write plain Ruby to define your application's configuration.
For the best development experience, be sure to use [RubyLSP](https://shopify.github.io/ruby-lsp/) for snippets, autocomplete and documentation, right in your editor.

{{< details title="An example Itsi.rb file:" >}}


```ruby {filename="Itsi.rb"}
workers 2

threads 2

fiber_scheduler true

auth_basic realm: "Restricted Area", credentials_file: "./credentials.txt"

auto_reload_config! # Auto-reload the server configuration each time it changes.

location "/app*" do
  rate_limit requests: 3, seconds: 5
  rackup_file "config.ru"
end

location "/inline*" do
  get "/" do |req|
    req.ok "Hello, World!"
  end
end
```
{{< /details >}}
