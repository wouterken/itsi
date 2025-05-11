require_relative 'lib/util'
require_relative 'lib/benchmark_case'
require_relative 'servers'
require 'etc'
require 'debug'
require 'json'
require 'fileutils'
require 'open3'
require 'socket'
require 'timeout'
require 'time'
require 'paint'

$LOAD_PATH.unshift(File.dirname(__FILE__))
$interrupt_signal = Queue.new

trap("INT") do
  exit(0) if $interrupt_signal.length > 0
  puts "\n[DEBUG] Caught Ctrl+C. Pausing before next iteration. Press again to exit."
  $interrupt_signal << true
end

def interrupted?
  $interrupt_signal.pop(true)
rescue
  false
end

def run_benchmark(test_name, test_case, server_config_file_path, server)
  puts Paint["\n=== Running #{test_name} on #{server.name}", :cyan, :bold]
  results = server.run!(server_config_file_path, test_case) do |url|
    warmup_cmd, test_cmds = \
      case test_case.tool
      when :wrk
        [
          "wrk -t4 -c50 -d#{test_case.warmup_duration}s #{url}",
          test_case.concurrency_levels.map do |level|
            [
              level,
              "wrk -t#{[level, 4].min} -c#{level} -d#{test_case.duration}s #{url}"
            ]
          end
        ]
      when :oha
        [
          "oha --no-tui -z #{test_case.warmup_duration}s -c50 #{url} -m #{test_case.method} #{test_case.data ? %{-d "#{test_case.data}"} : ""} -j #{test_case.http2? ? '--http2 --insecure' : ''} #{test_case.http2? ? "-p #{test_case.parallel_requests}" : ''}", # rubocop:disable Layout/LineLength
          test_case.concurrency_levels.map do |level|
            [
              level,
              "oha #{test_case.nonblocking ? '' : '-w'} --no-tui -z #{test_case.duration}s -c#{level} #{url} -m #{test_case.method} #{test_case.data ? %(-d "#{test_case.data}") : ""} -j #{test_case.http2? ? '--http2 --insecure' : ''} #{test_case.http2? ? "-p #{test_case.parallel_requests}" : ''}" # rubocop:disable Layout/LineLength
            ]
          end
        ]
      else
        raise "Unknown tool: #{test_case.tool}"
      end

    puts Paint["\nWarming up with:", :yellow, :bold]
    puts Paint[warmup_cmd, :yellow]
    `#{warmup_cmd}`

    test_cmds.map do |level, cmd|
      puts Paint["\nRunning #{test_case.tool} with concurrency level #{level}:", :blue, :bold]
      puts Paint[cmd, :blue]
      result_output = `#{cmd}`


      parsed_result = \
        case test_case.tool
        when :wrk
          parse_wrk_output(result_output)
        when :oha
          result_json = JSON.parse(result_output)
          summary = result_json['summary']
          p95_latency = result_json['latencyPercentiles']['p95']
          rps = summary['requestsPerSec'].round(2)
          failure = (1 - summary['successRate']).*(100.0).round(2)

          puts Paint % ['RPS: %{rps}. Errors: %{failure}%', :bold, :cyan,
                        {rps: [rps.to_s, :green], failure: [failure, failure.positive? ? :red : :green]}]

          error_distribution = result_json['errorDistribution']
          summary['errorDistribution'] = error_distribution

          if failure.positive? && error_distribution
            puts Paint["\nError breakdown:", :red, :bold]
            max_key_length = error_distribution.keys.map(&:length).max
            error_distribution.each do |err, count|
              padded_key = err.ljust(max_key_length)
              puts Paint["  #{padded_key} : #{count}", :red]
            end
          end

          summary.transform_values!{|v| v.is_a?(Numeric) ? v.round(2) : v }
          summary['p95_latency'] = p95_latency
          summary
        else
          result_output
        end

      binding.b if interrupted?

      [level, parsed_result]
    end.to_h
  end

  {
    server: server.name,
    test_case: test_name,
    results: results,
    timestamp: Time.now.utc.iso8601
  }
end

def cpu_label
  uname, = Open3.capture2('uname -m')
  arch = uname.strip

  model = \
    case RUBY_PLATFORM
    when /darwin/
      output, = Open3.capture2('sysctl -n machdep.cpu.brand_string')
      output.strip
    when /linux/
      model_name = File.read('/proc/cpuinfo')[/^model name\s+:\s+(.+)$/, 1]
      model_name || arch
    else
      arch
    end

  model.downcase.gsub(/[^a-z0-9]+/, '_').gsub(/^_|_+$/, '')
end

def save_result(result)
  device = cpu_label
  path_prefix = File.join("results", result[:test_case], device)
  FileUtils.mkdir_p(path_prefix)
  path = File.join(path_prefix, "#{result[:server].downcase}.json")
  File.write(path, JSON.pretty_generate(result))
end

filters = ARGV.map(&Regexp.method(:new))

Dir.glob('test_cases/*/*.rb').each do |path|

  test_name = File.basename(path, '.rb')
  begin
    src = IO.read(path).strip
    next if src.empty?

    test_case = BenchmarkCase.new(test_name)
    test_case.instance_eval(src)
    Server::ALL.each do |server|
      next if filters.any?{|filter| !(filter =~ server.name.to_s || filter =~ test_name.to_s) }
      next unless test_case.requires.all?{|r| server.supports?(r) }

      server_config_file_path = File.join(File.dirname(path), "#{server.name}.rb")
      server_config_file_path = "server_configurations/#{server.name}.rb" unless File.exist?(server_config_file_path)

      raise "Couldn't find server config file for #{server.name}" unless File.exist?(server_config_file_path)

      result = run_benchmark(test_name, test_case, server_config_file_path, server)
      save_result(result)
    end
  rescue StandardError => e
    puts "Error during test case #{path}. #{e}"
    puts e.backtrace
  end
end
