# frozen_string_literal: true

large = `ruby --help` * 1000 # 3MB

run(
  proc do
    [200, {"content-type" => "text/plain"}, [large]]
  end
)
