---
title: Watch
url: /options/watch
---

The `watch` option uses Itsi's built-in file watching mechanism to execute a list of commands each time any file matching the path glob parameter is modified.

You can have several active watches at once.

E.g.

```ruby {filename=Itsi.rb}
watch 'config/*.yml', [%w[npm run build], %w[bundle exec cache:mark_dirty]]

watch 'app/**/*.rb', [%w[bundle exec itsi restart]]
```
