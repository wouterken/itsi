---
title: Route Testing
type: docs
---
Itsi provides convenience functions to verify which middleware is applied to a route.
Use the following CLI command to test a route:

```bash
itsi test_route /admin
```

```bash
Route:      /admin
Conditions: extensions: html,css,js,png,jpg,
Middleware: • log_requests(before: I am th..., after: [{reque...)
           • compress(zstd gzip deflate br, ["all"])
           • cors(*, GET POST PUT DELETE)
           • etag(strong/md5, if_none_match)
           • cache_control(max_age: 3600, public, private, no_cache, no_store, must_revalidate, proxy_revalidate, immutable)
           • app(/Users/pico/Development/itsi/gems/server/lib/itsi/server/rack_interface.rb:1)
           • static_assets(path: ./)
```

You can also print out **all** currently configured routes in your `Itsi.rb` using
```bash
itsi routes
```
