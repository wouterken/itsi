require 'debug'
require "osprey/scheduler"

run lambda { |env|
  # 5.times do |i|
  #   puts "#{i}"
  #   sleep 0.3
  # end
  [200, { 'Content-Type' => 'text/plain' }, ['foo']]
}
