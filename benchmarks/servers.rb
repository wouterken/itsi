# frozen_string_literal: true

require_relative 'lib/server'

Server.new \
  :falcon,
  '%<base>s -b %<scheme>s://%<host>s:%<port>s -c %<app_path>s --hybrid --forks %<workers>s --threads %<threads>s',
  supports: %i[http2 threads processes streaming_body static ruby]

Server.new \
  :iodine,
  '%<base>s -p %{port} %{app_path} -w %<workers>s -t %{threads} %{www}',
  supports: %i[threads processes static ruby],
  www: ->(test_case, _args){  test_case.static_files_root ? "-www #{test_case.static_files_root}" : ""}

Server.new \
  :itsi,
  '%{base} -C %{config} -b %{scheme}://%{host}:%{port} --rackup_file=%{app_path} -w %{workers} -t %{threads} %{scheduler_toggle}',
  supports: %i[http2 threads processes streaming_body static ruby grpc],
  scheduler_toggle: ->(test_case, _args){ test_case.nonblocking ? "-f" : "" }

Server.new \
  :puma,
  '%{base} %{config}-b tcp://%{host}:%{port} %{app_path} -w %{workers} -t %{threads}:%{threads}',
  supports: %i[http1 threads processes streaming_body static ruby],
  config: ->(_, args){ File.exist?(args[:config]) ? "-C #{args[:config]} " : "" }

Server.new \
  :unicorn,
  'UNICORN_WORKERS=%{workers} %{base} %{config}-l %{host}:%{port} %{app_path}',
  supports: %i[processes streaming_body static ruby],
  config: ->(_, args){ File.exist?(args[:config]) ? "-c #{args[:config]} " : "" }

Server.new \
  :agoo,
  '(cd apps && %{base} -p %{port} %{app_path} -w %{workers} -t %{threads} %{www})',
  supports: %i[threads processes streaming_body static ruby],
  www: ->(test_case, _args){  test_case.static_files_root ? "-d #{test_case.static_files_root}" : ""},
  app_path: ->(_test_case, args){ args[:app_path].gsub("apps/", "") }

Server.new \
  :nginx,
  "nginx -p \"#{Dir.pwd}\" -c %{config_file}",
  supports: %i[static http2],
  config_file: ->(_, args){
    temp_config = Tempfile.new(['nginx', '.conf'])
    temp_config.write(IO.read('server_configurations/nginx.conf') % args)
    temp_config.flush
    temp_config.path
  }

Server.new \
  :caddy,
  'GOMAXPROCS=%{workers} caddy file-server --listen %{host}:%{port} --browse --root %<www>s',
  supports: %i[static http2]

Server.new \
  :h2o,
  'h2o -c %<config_file>s',
  supports: %i[static http2],
  config_file: lambda { |_, args|
    temp_config = Tempfile.new(['nginx', '.conf'])
    temp_config.write(IO.read('server_configurations/nginx.conf') % args)
    temp_config.flush
    temp_config.path
  }

Server.new \
  :"grpc_server.rb",
  'bundle exec ruby ./grpc_server.rb',
  supports: %i[grpc http2]
