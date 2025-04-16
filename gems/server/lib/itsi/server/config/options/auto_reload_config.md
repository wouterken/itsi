---
title: Auto Reload Config
url: /options/auto_reload_config
weight: 3
---

Auto reload config is a feature that allows the server to automatically reload the configuration file when it is modified. This feature is useful when you want to make changes to the configuration file without having to restart the server.

To opt in to config auto reloading, just add the following line to your configuration file:

```ruby {filename=Itsi.rb}
auto_reload_config!
```
