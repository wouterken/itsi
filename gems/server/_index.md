---
title: Features
type: docs
next: /features/server/lib/itsi/server/config/options/workers/
url: /features
cascade:
  type: docs
---

Itsi is a powerful, flexible, and easy-to-use server-side framework for building web applications.

## Hello, World!

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
