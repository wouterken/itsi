---
title: Preload
url: /options/preload
---

The **preload** option controls whether your application code and middleware are loaded into memory **before** forking worker processes. Preloading can improve request‑handling performance (thanks to copy‑on‑write memory sharing and avoiding per‑worker load time), but it also increases the master process’s memory footprint and startup time. In **single‑worker** mode (`workers 1`), `preload` is ignored.

## Configuration File

```ruby
# Load everything in the master process before forking
preload true

# Only preload the gems in the named Gemfile group (e.g. :assets)
preload :assets
# Only preload the gems in the named Gemfile group (e.g. :preload)
preload :preload

# Do not preload (load after forking in each worker)
preload false
```
