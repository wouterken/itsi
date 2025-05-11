# lib/benchmark_case.rb
class BenchmarkCase
  %i[
    name description app method data path
    workers threads warmup_duration concurrency_levels
    tool duration https parallel_requests nonblocking
    requires
  ].each do |accessor|
    define_method(accessor) do |value = self|
      if value.eql?(self)
        instance_variable_get("@#{accessor}")
      else
        instance_variable_set("@#{accessor}", value)
      end
    end
  end

  def initialize(name) # rubocop:disable Metrics/AbcSize,Metrics/MethodLength
    @name = name
    @description = ''
    @method = 'GET'
    @data = nil
    @path = '/'
    @tool = :oha
    @workers = 1
    @threads = 1
    @duration = ENV.fetch('RACK_BENCH_DURATION_SECONDS', 5).to_i
    @warmup_duration = ENV.fetch('RACK_BENCH_WARMUP_DURATION_SECONDS', 1).to_i
    @concurrency_levels = [10, 50, 100, 250]
    @wrk_threads = 2
    @https = false
    @parallel_requests = 16
    @nonblocking = false
    @requires = []
    yield self if block_given?
  end

  def method_missing(name, *args, **kwargs, &blk)
    return super unless name.end_with?('?')

    @requires.include?(name[0...-1].to_sym)
  end

  def respond_to_missing?(name, include_private = false)
    @requires.include?(name[0...-1].to_sym) || super
  end
end
