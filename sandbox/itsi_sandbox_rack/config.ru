require 'itsi/scheduler'
require 'async'
require 'debug'
def looper
  i = 0
  loop do
    i += 1
    yield "#{i}\n"
  end
end

require 'itsi/server'

memory_leak = []

run lambda { |env|
  [200, { 'Content-Type' => 'text/plain' }, ['foo']]
}
