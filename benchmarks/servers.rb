require_relative "lib/server"

Server.new \
  :falcon,
  '%{base} -b %{scheme}://%{host}:%{port} -c %{app_path} --hybrid --forks %{workers} --threads %{threads}',
  supports: %i[http2 threads processes streaming_body]


Server.new \
  :iodine,
  '%{base} -p %{port} %{app_path} -w %{workers} -t %{threads}',
  supports: %i[threads processes]


Server.new \
  :itsi,
  '%{base} -C %{config} -b %{scheme}://%{host}:%{port} --rackup_file=%{app_path} -w %{workers} -t %{threads} %{scheduler_toggle}',
  supports: %i[http2 threads processes streaming_body],
  scheduler_toggle: ->(test_case){ test_case.nonblocking ? "-f" : "" }


Server.new \
  :puma,
  '%{base} -C %{config} -b tcp://%{host}:%{port} %{app_path} -w %{workers} -t %{threads}:%{threads}',
  supports: %i[http1 threads processes streaming_body]


Server.new \
  :unicorn,
  'UNICORN_WORKERS=%{workers} %{base} -c %{config} -l %{host}:%{port} %{app_path}',
  supports: %i[processes streaming_body]


Server.new \
  :agoo,
  '%{base} -p %{port} %{app_path} -w %{workers} -t %{threads}',
  supports: %i[threads processes streaming_body]
