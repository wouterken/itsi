source 'https://rubygems.org'

gemspec
gemspec path: 'gems/server'
gemspec path: 'gems/scheduler'

group :test do
  gem 'activerecord'
  gem 'jwt'
  gem 'net_http_unix'
  gem 'pg'
  gem 'redis'
end

group :development, :test do
  gem 'bundler'
  gem 'irb'
  gem 'minitest', '~> 5.16'
  gem 'minitest-reporters'
  gem 'rack'
  gem 'rackup'
  gem 'rake', '~> 13.0'
  gem 'rake-compiler'
  gem 'rb_sys', '~> 0.9.91'
  gem 'rubocop', '~> 1.21'
end

group :server_test do
  gem 'grpc'
end

group :sandbox do
  gem 'falcon'
  gem 'iodine'
end

group :tools do
  gem 'debug'
  gem 'ruby-lsp'
  gem 'solargraph'
end
