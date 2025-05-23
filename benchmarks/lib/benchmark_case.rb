# lib/benchmark_case.rb
class BenchmarkCase

  # Give the server some time to warm up before we start measuring.
  RACK_BENCH_WARMUP_DURATION_SECONDS = ENV.fetch('RACK_BENCH_WARMUP_DURATION_SECONDS', 1).to_i

  # A default 3 is relatively low, but allows us to get through the entire test
  # suite quickly. For a more robust benchmark, this should be higher.
  RACK_BENCH_DURATION_SECONDS = ENV.fetch('RACK_BENCH_DURATION_SECONDS', 3).to_i

  %i[
    name description app method data path
    workers threads warmup_duration concurrency_levels
    duration https parallel_requests nonblocking
    requires use_yjit static_files_root grpc call proto
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
    @proto = ""
    @call = ""
    @workers = 1
    @threads = [1, 5, 10, 20]
    @workers = [1, 2, Etc.nprocessors].uniq
    @duration = RACK_BENCH_DURATION_SECONDS
    @warmup_duration = RACK_BENCH_WARMUP_DURATION_SECONDS
    @concurrency_levels = [10, 50, 100, 250]
    @static_files_root = nil
    @https = false
    @grpc = false
    @parallel_requests = 16
    @nonblocking = false
    @requires = %i[ruby]
    @use_yjit = true
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
