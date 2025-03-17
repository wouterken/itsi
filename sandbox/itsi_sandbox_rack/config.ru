require 'itsi/scheduler'
response = [200, { 'Content-Type' => 'text/plain' }, ['foo']].freeze
run lambda { |env|
  response
}
