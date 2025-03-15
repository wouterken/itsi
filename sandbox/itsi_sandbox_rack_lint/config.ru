require 'uri'
use Rack::Lint
run ->(env) { [200, { 'content-type' => 'text/plain' }, ['Hello, World!']] }
