# frozen_string_literal: true

run(
  proc do
    sleep 1
    [200, { 'content-type' => 'text/plain' }, ['hello world']]
  end
)
