---
title: Configuration
type: docs
weight: 3
next: options/
prev: getting_started/
---

## Itsi.rb
To realize the full power of all of Itsi's features, you'll typically create a configuration file
(usually named `Itsi.rb`) at the root of your project.<br/>
If you're ready to get stuck in and learn all about what Itsi has to offer, use
`itsi init` to generate a fresh configuration file and read through the following [options](/options/) and [middleware](/middleware/) sections.


## Out-of-the-box
If you prefer a more gradual introduction, Itsi provides several out-of-the box capabilities that you can take advantage of immediately, *without* needing to create a dedicated configuration file.



## Run Rack Applications
Itsi will automatically host your Rack application if you launch it in a directory with a `config.ru` file.
This means, it's a drop-in server replacement (and potentially a free performance boost) for your favorite `Rails`, `Hanami`, or `Sinatra` applications.

To get started just run
```ruby
itsi # or
bundle exec itsi
```

There's also a Rails adapter allowing you to add it to your Gemfile and launch it using the `rails server` command.

```ruby
rails server -u itsi
```
{{< callout type="info" >}}
  Note that `rails server -u itsi` runs Itsi with an intentionally minimal footprint, specifically for development purposes. To take full advantage of Itsi's concurrency features,
it's advised you tweak these inside a dedicated `Itsi.rb` file.
  {{< /callout >}}



## Host static files
You can run Itsi as a fully fledged, static file server.
With a single command, Itsi will start an HTTP server to serve files from the current directory.
To get started just run.
```ruby
itsi static # or
bundle exec itsi static
```

> This starts a server with a minimal set of defaults. Look at the [`static_assets`](/middleware/static_assets) middleware page to learn more about how to configure Itsi for full control over static file server capabilities.

## Tweak your Itsi server
Several of the most common features of Itsi are configurable using command line flags.
Run `itsi --help` to see all available options. If you apply both command line flags and an `Itsi.rb` config file, the command line flags will take precedence.

```bash
❯ itsi --help
Usage: itsi [COMMAND] [options]
    -C, --config CONFIG_FILE         Itsi Configuration file to use (default: Itsi.rb)
    -w, --workers WORKERS            Number of workers
    -d, --daemonize                  Run the process as a daemon
    -t, --threads THREADS            Number of threads (default: 1)
        --[no-]multithreaded-reactor Use a multithreaded reactor
    -r, --rackup_file FILE           Rackup file to use (default: config.ru)
        --worker-memory-limit MEMORY_LIMIT
                                     Memory limit for each worker (default: None). If this limit is breached the worker is gracefully restarted
    -f [CLASS_NAME],                 Scheduler class to use (default: nil). Provide blank or true to use Itsi::Scheduler, or a classname to use an alternative scheduler
        --fiber_scheduler
        --preload [true, false, :bundle_group_name]
                                      Toggle preloading the application
    -b, --bind BIND                  Bind address (default: http://0.0.0.0:3000). You can specify this flag multiple times to bind to multiple addresses.
    -c, --cert_path CERT_PATH        Path to the SSL certificate file (must follow a --bind option). You can specify this flag multiple times.
    -k, --key_path KEY_PATH          Path to the SSL key file (must follow a --bind option). You can specify this flag multiple times.
        --shutdown_timeout SHUTDOWN_TIMEOUT
                                     Graceful timeout period before forcing workers to shutdown
        --stream-body                Stream body frames (default: false for best compatibility)
    -h, --help                       Show this help message
        --reexec PARAMS              Reexec the server with the given parameters
        --listeners LISTENERS        Listeners for reexec
        --passfile PASSFILE          Passfile
        --algorithm ALGORITHM        Algorithm for password hashing
COMMAND:
    init - Initialize a new Itsi.rb server configuration file
    status - Show the status of the server
    start - Start the Itsi server
    serve - Start the Itsi server
    stop - Stop the server
    reload - Reload the server
    restart - Restart the server
    add_worker - Add a new worker to the server cluster
    remove_worker - Remove a worker from the server cluster
    test - Test config file validity
    routes - Print the routes of the server
    passfile - Manage hashed users and passwords in a passfile (like .htpasswd). [add, echo, remove, list]
    secret - Generate a new secret for use in a JWT verifier
    test_route - Test which route a request will be routed to
    static - Serve static assets in the given directory

```
