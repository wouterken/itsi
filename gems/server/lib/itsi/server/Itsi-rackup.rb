# frozen_string_literal: true

# This is the default Itsi configuration file, installed when you run `itsi init`
# It contains a sane starting point for configuring your Itsi server.
# You can use this file in both development and production environments.
# Most of the options in this file can be overridden by command line options.
# Check out itsi -h to learn more about the command line options available to you.

env = ENV.fetch("APP_ENV") { ENV.fetch("RACK_ENV", "development") }

# Number of worker processes to spawn
# If more than 1, Itsi will be booted in Cluster mode
workers ENV.fetch("ITSI_WORKERS") {
  require "etc"
  env == "development" ? 1 : Etc.nprocessors
}

# Number of threads to spawn per worker process
# For pure CPU bound applicationss, you'll get the best results keeping this number low
# Setting a value of 1 is great for superficial benchmarks, but in reality
# it's better to set this a bit higher to allow expensive requests to get overtaken and minimize head-of-line blocking
threads ENV.fetch("ITSI_THREADS", 3)

# If your application is IO bound (e.g. performing a lot of proxied HTTP requests, or heavy queries etc)
# you can see *substantial* benefits from enabling this option.
# To set this option, pass a string, not a class (as we will not have loaded the class yet)
# E.g.
# `fiber_scheduler "Itsi::Scheduler"` - The default fast and light-weight scheduler that comes with Itsi
# `fiber_scheduler "Async::Scheduler"` - Bring your own scheduler!
fiber_scheduler nil

# By default Itsi will run the Rack app from config.ru.
# You can provide an alternative Rack app file name here
# Or you can inline the app directly inside Itsi.rb.
# Only one of `run` and `rackup_file` can be used.
# E.g.
# require 'rack'
# run(Rack::Builder.app do
#   use Rack::CommonLogger
#   run ->(env) { [200, { 'content-type' => 'text/plain' }, ['OK']] }
# end)
rackup_file "config.ru"

# If you bind to https, without specifying a certificate, Itsi will use a self-signed certificate.
# The self-signed certificate will use a CA generated for your host and stored inside `ITSI_LOCAL_CA_DIR` (Defaults to ~/.itsi)
# bind "https://localhost:3000"
# bind "https://localhost:3000?domains=dev.itsi.fyi"
#
# If you want to use let's encrypt to generate you a real certificate you and pass cert=acme and an acme_email address to generate one.
# bind "https://itsi.fyi?cert=acme&acme_email=admin@itsi.fyi"
# You can generate certificates for multiple domains at once, by passing a comma-separated list of domains
# bind "https://0.0.0.0?domains=foo.itsi.fyi,bar.itsi.fyi&cert=acme&acme_email=admin@itsi.fyi"
#
# If you already have a certificate you can specify it using the cert and key parameters
# bind "https://itsi.fyi?cert=/path/to/cert.pem&key=/path/to/key.pem"
#
# You can also bind to a unix socket or a tls unix socket. E.g.
# bind "unix:///tmp/itsi.sock"
# bind "tls:///tmp/itsi.secure.sock"

if env == "development"
  bind "http://localhost:3000"
else
  bind "https://0.0.0.0?domains=#{ENV["PRODUCTION_DOMAINS"]}&cert=acme&acme_email=admin@itsi.fyi"
end

# If you want to preload the application, set preload to true
# to load the entire rack-app defined in rack_file_name before forking.
# Alternatively, you can preload just a specific set of gems in a group in your gemfile,
# by providing the group name here.
# E.g.
#
# preload :preload # Load gems inside the preload group
# preload false # Don't preload.
#
# If you want to be able to perform zero-downtime deploys using a single itsi process,
# you should disable preloads, so that the application is loaded fresh each time a new worker boots
preload true

# Set the maximum memory limit for each worker process in bytes
# When this limit is reached, the worker will be gracefully restarted.
# Only one worker is restarted at a time to ensure we don't take down
# all of them at once, if they reach the threshold simultaneously.
worker_memory_limit 1024 * 1024 * 1024

# You can provide an optional block of code to run, when a worker hits its memory threshold (Use this to send yourself an alert,
# write metrics to disk etc. etc.)
after_memory_threshold_reached do |pid|
  puts "Worker #{pid} has reached its memory threshold and will restart"
end

# Do clean up of any non-threadsafe resources before forking a new worker here.
before_fork {}

# Reinitialize any non-threadsafe resources after forking a new worker here.
after_fork {}

# Shutdown timeout
# Number of seconds to wait for workers to gracefully shutdown before killing them.
shutdown_timeout 5

# Set this to false for application environments that require rack.input to be a rewindable body
# (like Rails). For rack applications that can stream inputs, you can set this to true for a more memory-efficient approach.
stream_body false

# OOB GC responses threshold
# Specifies how frequently OOB gc should be triggered during periods where there is a gap in queued requests.
# Setting this too low can substantially worsen performance
oob_gc_responses_threshold 512

# Log level
# Set this to one of the following values: debug, info, warn, error, fatal
# Can also be set using the ITSI_LOG environment variable
log_level :info

# Log Format
# Set this to be either :ansi or :json. If you leave it blank Itsi will try
# and auto-detect the format based on the TTY environment.
log_format :auto
