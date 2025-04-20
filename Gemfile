source 'https://rubygems.org'

gemspec
gemspec path: 'gems/server'
gemspec path: 'gems/scheduler'

group :test do
  gem 'activerecord'
  gem 'pg'
  gem 'net_http_unix'
  gem 'jwt'
  gem 'redis'
end

group :development, :test do
  gem 'bundler'
  gem 'debug'
  gem 'irb'
  gem 'minitest', '~> 5.16'
  gem 'minitest-reporters'
  gem 'rake', '~> 13.0'
  gem 'rake-compiler'
  gem 'rubocop', '~> 1.21'
  gem 'ruby-lsp'
  gem 'solargraph'
  gem 'iodine'
  gem 'falcon'
  gem 'rb_sys', '~> 0.9.91'
  gem "grpc"
end
