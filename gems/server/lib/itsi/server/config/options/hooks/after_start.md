---
title: After Start
url: /options/after_start
---

The **after_start** hook runs once after the server start-up process has completed.

```ruby {filename=Itsi.rb}
after_start do
  DB.reconnect!
end
```
