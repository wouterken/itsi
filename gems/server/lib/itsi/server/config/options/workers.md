---
title: Workers
url: /options/workers
next: middleware/
---
Itsi is a preforking server. It can run as either a single process (`workers 1`) or in clustered mode (`workers > 1`).
<br/>You can switch between the two without downtime.

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
to be at or near the number of CPU cores available on your system.

However, note hyperthreads and efficiency cores may impact this and using every available core is not
always the most effective choice.

## Command Line
You can also override the number of workers using either the `-w` or workers `--workers` command line option.
E.g.

```bash
itsi -w 4
```
