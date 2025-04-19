---
title: Rackup File
url: /middleware/rackup_file
---

You can use the `rackup_file` middleware to mount a rack application defined in a Rackup file (typically `config.ru`)

The alternative way to mount a rack application is to use the [`run`](/middleware/run) middleware to define a Rack app inline.

You can mount several rack applications to run simultaneously within a single Itsi application, using different location blocks.
Depending on *where* you mount the app, the application will receive different values for `PATH_INFO`, `SCRIPT_NAME`.


{{< callout  >}}
If no `Itsi.rb` file is defined, Itsi will attempt to run the app defined at `./config.ru` by default, just like other Rack servers. You can also use `-r` or `--rackup_file` flags to specify a different file.
{{< /callout >}}


## Configuration

### Simple inline Rackup file.
```ruby {filename=Itsi.rb}
rackup_file "config.ru"
```


### Rack app mounted at a subpath
```ruby {filename=Itsi.rb}
require 'rack'
location "/subpath/*" do
  rackup_file "config.ru"
end

rackup_file "config.ru"

```

```bash
# SCRIPT_NAME is "/subpath", path_info is "/child_path"
$ curl http://0.0.0.0:3000/subpath/child_path

# SCRIPT_NAME is "", path_info is "/root/child_path"
$ curl http://0.0.0.0:3000/root/child_path
:/root/child_path
```

### Options
* `nonblocking` (default false). Determines whether requests sent to this Rack application should be run on non-blocking threads. Only applies if running in hybrid (non-blocking and blocking thread pool) mode. Otherwise this is a no-op and will run in whatever mode is set globally.
* `sendfile` (default true). Determines whether Itsi should respect the `X-Sendfile` header set by the Rack application and use the `sendfile` function to efficiently send files. (Despite the name, this does not use the OS-level `sendfile` system call). Note. Itsi enforces the restriction that the referenced file must be within a child directory of the application root.

e.g.
```ruby {filename=Itsi.rb}
rackup_file "config.ru", nonblocking: true, sendfile: false
```
