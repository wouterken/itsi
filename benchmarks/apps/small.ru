# frozen_string_literal: true

small = `ruby --help` # ~3kb

run(
  proc do
    [200, {'content-type' => 'text/plain'}, [small]]
  end
)
