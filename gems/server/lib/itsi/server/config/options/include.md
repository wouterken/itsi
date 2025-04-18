---
title: Include
url: /options/include
---

Use the `include` option to load additional files to be evaluated within the current configuration context.
You can use this option to split a large configuration file into multiple smaller files.


## Examples
```ruby {filename="Itsi.rb"}
include "middleware"
include "logging"
```

```ruby {filename="Itsi.rb"}
include "concurrency"
include "security"
```
