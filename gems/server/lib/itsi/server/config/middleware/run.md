---
title: Run
url: /middleware/run
---

You can use the `run` middleware to run an inline rack application.
The alternative way to mount a rack application is to use a [`rackup_file`](/middleware/rackup_file) middleware.

You can mount several rack applications  to run simultaneously within a single Itsi application, using different location blocks.
Depending on *where* you mount the app, the application will receive different values for `PATH_INFO`, `SCRIPT_NAME`.

## Configuration

### Simple inline Rack app.
```ruby {filename=Itsi.rb}
run ->(env){ [200, { 'content-type' => 'text/plain' }, ['OK']] }
```

### Rack app using Rack::Builder
```ruby {filename=Itsi.rb}
run(Rack::Builder.app do
  use Rack::CommonLogger
  run ->(env) { [200, { 'content-type' => 'text/plain' }, ['OK']] }
end)
```


### Rack app mounted at a subpath
```ruby {filename=Itsi.rb}
require 'rack'
location "/subpath/*" do
  run(Rack::Builder.app do
    use Rack::CommonLogger
    run ->(env) { [200, { 'content-type' => 'text/plain' }, [[env['SCRIPT_NAME'], env['PATH_INFO']].join(":") ]  ] }
  end)
end

run(Rack::Builder.app do
  use Rack::CommonLogger
  run ->(env) { [200, { 'content-type' => 'text/plain' }, [[env['SCRIPT_NAME'], env['PATH_INFO']].join(":") ]  ] }
end)

```

```bash
$ curl http://0.0.0.0:3000/subpath/child_path
/subpath:/child_path

$ curl http://0.0.0.0:3000/root/child_path
:/root/child_path
```

### Options
* `nonblocking` (default false). Determines whether requests sent to this Rack application should be run on non-blocking threads. Only applies if running in hybrid (non-blocking and blocking thread pool) mode. Otherwise this is a no-op and will run in whatever mode is set globally.
* `sendfile` (default true). Determines whether Itsi should respect the `X-Sendfile` header set by the Rack application and use the `sendfile` function to efficiently send files. (Despite the name, this does not use the OS-level `sendfile` system call). Note. Itsi enforces the restriction that the referenced file must be within a child directory of the application root.

e.g.
```ruby {filename=Itsi.rb}
run ->(env){ [200, { 'content-type' => 'text/plain' }, ['OK']] }, nonblocking: true, sendfile: false
```
