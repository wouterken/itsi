# Rack static handler. Fallback for servers that do not have built-in static asset serving.
# * Do have static asset serving: nginx, caddy, h2o, itsi, agoo, iodine
# * Don't have static asset serving: pume, falcon, unicorn
use Rack::Static, urls: ["/public"], root: File.expand_path("public", __dir__), index: 'index.html'

# Simulate static assets and slower Ruby endpoints working together
endpoint_delay = 0.01

run lambda { |env|
  sleep endpoint_delay
  [200, { "Content-Type" => "text/plain" }, ["Busy Endpoint: #{env['PATH_INFO']}"]]
}
