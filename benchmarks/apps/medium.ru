# frozen_string_literal: true

medium = `ruby --help` * 100 # 300kb

run(
  proc do
    [200, {"content-type" => "text/plain"}, [medium]]
  end
)
