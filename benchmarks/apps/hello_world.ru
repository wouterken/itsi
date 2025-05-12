# frozen_string_literal: true

run(
  proc do
    [200, { 'content-type' => 'text/plain' }, ['hello world']]
  end
)
