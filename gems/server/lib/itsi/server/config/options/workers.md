---
title: Workers
url: /features/workers
prev: /features
weight: 1
---
Itsi is a preforking server. It can run as a single process or in cluster mode.
You can switch between the two without downtime.

## Configuration File
The number of workers to use can be specified inside the configuration file (usually `./Itsi.rb` at the project root)
using the `workers` function.

## Examples
```ruby {filename="Itsi.rb"}
# Starts a single worker, putting Itsi in single-process mode
workers 1
```

```ruby {filename="Itsi.rb"}
# Starts 4 workers, putting Itsi in cluster-process mode
workers 4
```

```ruby {filename="Itsi.rb"}
# Sets the number of workers to the number of CPU cores available on the system
workers Etc.nprocessors
```

To maximize performance, it's typical to increase number of workers
to match the number of CPU cores available on your system.

## Command Line
You can also override the number of workers using either the `-w` or workers `--workers` command line option.
E.g.

```bash
itsi -w 4
```
