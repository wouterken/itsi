---
title: Multithreaded Reactor
url: /options/multithreaded_reactor
---

Configures whether the Tokio reactor should run in multithreaded mode.

Itsi will attempt to intelligently determine this config value by default, by enabling it in single mode, and disabling it in cluster-mode (where attempting to utilize multiple cores via both multi-threaded reactor and multi-worker is generally less efficient, not recommended).
however you can use this option to override it.

### Default

```ruby {filename=Itsi.rb}
multithreaded_reactor :auto # Default. Will result in true if running in single mode, false if running in multi mode.
```
### Example

```ruby {filename=Itsi.rb}
multithreaded_reactor true
```
