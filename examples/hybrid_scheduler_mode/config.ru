require 'net/http'

# This could be any Rack app, a Rails app, a Sinatra app, etc.
# We're going to use a simple inline rack handler for now.

run ->(_) do
  result = Net::HTTP.new('127.0.0.1', 3005).get('/?sleep=2')
  [result.code.to_i, {'content-type' => 'text/html'}, [result.body]]
end
