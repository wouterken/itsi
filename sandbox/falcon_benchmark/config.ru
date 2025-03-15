require 'itsi/scheduler'
require 'itsi/server'

class Benchmark
	def initialize(app)
		@app = app
	end

	PATH_INFO = 'PATH_INFO'.freeze

	SMALL = [200, {}, ["Hello World\n" * 10] * 10].freeze

	def blocking(env)
		Kernel::sleep(0.01)

		SMALL
	end

	def nonblocking(env)
		if defined? Async
			Async::Task.current.sleep(0.01)
		else
			Kernel::sleep(0.01)
		end

		SMALL
	end

	def small(env)
		SMALL
	end

	LARGE = [200, {}, ["Hello World\n" * 100] * 100].freeze

	def large(env)
		LARGE
	end

	def call(env)
		_, name, *path = env[PATH_INFO].split("/")

		method = name&.to_sym

		if method and self.respond_to?(method)
			self.send(method, env)
		else
			@app.call(env)
		end
	end
end

use Benchmark

run lambda {|env| [404, {}, ["Not Found"]]}
