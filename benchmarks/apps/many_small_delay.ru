# frozen_string_literal: true

run(
  proc do
    100.times do
      sleep 0.01
    end
    [200, { 'content-type' => 'text/plain' }, ['hello world']]
  end
)
