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
require 'uri'

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

def run_benchmark(
  test_name, test_case, server_config_file_path, server,
  threads:, workers:, http2:
)
  server.run!(server_config_file_path, test_case, threads, workers) do |workloads|
    puts Paint["\n=== Running #{test_name} on #{server.name}", :cyan, :bold]
    warmup_cmds = workloads.map do |url, method, data|
      if test_case.grpc?
        "ghz --duration-stop=ignore --cpus=2 -z #{test_case.warmup_duration}s -c50 --call #{test_case.call} --stream-call-count=5 -d #{data} --insecure #{URI(url).host}:#{URI(url).port} --proto #{test_case.proto} -O json"
      else
        "oha --no-tui -z #{test_case.warmup_duration}s -c50 #{url} -m #{method} #{data ? %{-d "#{data}"} : ""} -j #{http2 ? '--http2 --insecure' : ''} #{http2 ? "-p #{test_case.parallel_requests}" : ''}" # rubocop:disable Layout/LineLength
      end
    end

    test_command_sets = test_case.concurrency_levels.map do |level|
      [
        level,
        workloads.map do |url, method, data|
          if test_case.grpc?
            "ghz --duration-stop=ignore --cpus=2 -z #{test_case.duration}s -c#{level} --call #{test_case.call} --stream-call-count=5 -d #{data} --insecure #{URI(url).host}:#{URI(url).port} --proto #{test_case.proto} -O json"
          else
            "oha #{test_case.nonblocking ? '' : '-w'} --no-tui -z #{test_case.duration}s -c#{level} #{url} -m #{method} #{data ? %{-d "#{data}"} : ""} -j #{http2 ? '--http2 --insecure' : ''} #{http2 ? "-p #{test_case.parallel_requests}" : ''}" # rubocop:disable Layout/LineLength
          end
        end
      ]
    end

    puts Paint["\nWarming up with:", :yellow, :bold]
    warmup_cmds.each do |warmup_cmd|
      puts Paint[warmup_cmd, :yellow]
      `#{warmup_cmd}`
    end

    test_command_sets.map do |concurrency, cmds|
      puts Paint["\n[#{test_case.name}] #{server.name}(#{workers}x#{threads}). Concurrency #{concurrency}. #{http2 ? "HTTP/2.0": "HTTP/1.1"}", :blue, :bold]
      result_outputs = cmds.map do |cmd|
        puts Paint[cmd, :blue]
        Thread.new{ `#{cmd}` }
      end.map(&:value)

      result_outputs.map! do |result_output|
        if test_case.grpc?
          result_json = JSON.parse(result_output)
          success_rate = ((result_json["statusCodeDistribution"]&.[]("OK") / result_json["count"].to_f).round(4) rescue 0)

          gross_rps = (result_json["rps"]).round(4)
          failure_rate = (1 - success_rate)
          net_rps = (gross_rps * (1 - failure_rate)).round(2)
          failure = failure_rate.*(100.0).round(2)

          error_distribution = result_json['errorDistribution']

          if failure.positive? && error_distribution
            puts Paint["• Error breakdown:", :red, :bold]
            max_key_length = error_distribution.keys.map(&:length).max
            error_distribution.each do |err, count|
              padded_key = err.ljust(max_key_length)
              puts Paint["  #{padded_key} : #{count}", :red]
            end
          end

          puts Paint % ['RPS: %{rps}. Errors: %{failure}%', :bold, :cyan,
                        {rps: [net_rps.to_s, :green], failure: [failure, failure.positive? ? :red : :green]}]
          {
            "successRate" => success_rate,
            "total" => (result_json["count"]),
            "slowest" => (result_json["slowest"]).round(4),
            "fastest" => (result_json["fastest"] == Float::INFINITY ? 0.0 : result_json["fastest"]).round(4),
            "average" => result_json["average"].round(4),
            "requestsPerSec" => net_rps,
            "grossRequestsPerSec" => gross_rps,
            "errorDistribution" => error_distribution,
            "p95_latency" =>  result_json["latencyDistribution"].find{|ld| ld["percentage"] == 95 }["latency"] / (1000.0 ** 2)
          }
        else
          result_json = JSON.parse(result_output)
          summary = result_json['summary']
          p95_latency = result_json['latencyPercentiles']['p95']
          gross_rps = summary['requestsPerSec'].round(2)
          failure_rate = (1 - summary['successRate'])
          net_rps = (gross_rps * (1 - failure_rate)).round(2)
          failure = failure_rate.*(100.0).round(2)

          puts Paint % ['RPS: %{rps}. Errors: %{failure}%', :bold, :cyan,
                        {rps: [net_rps.to_s, :green], failure: [failure, failure.positive? ? :red : :green]}]

          error_distribution = result_json['errorDistribution']
          summary['errorDistribution'] = error_distribution
          summary["requestsPerSec"] => net_rps
          summary["grossRequestsPerSec"] => gross_rps

          if failure.positive? && error_distribution
            puts Paint["• Error breakdown:", :red, :bold]
            max_key_length = error_distribution.keys.map(&:length).max
            error_distribution.each do |err, count|
              padded_key = err.ljust(max_key_length)
              puts Paint["  #{padded_key} : #{count}", :red]
            end
          end

          summary.transform_values!{|v| v.is_a?(Numeric) ? v.round(2) : v }
          summary['p95_latency'] = p95_latency
          summary
        end
      end

      binding.b if interrupted? # rubocop:disable Lint/Debugger

      {
        server: server.name,
        test_case: test_name,
        threads: threads,
        workers: workers,
        http2: http2,
        concurrency: concurrency,
        **(workers == 1 ? {rss_mb: (server.rss / (1024.0 * 1024.0)).round(2) } : {}),
        results: combine_results(result_outputs),
        timestamp: Time.now.utc.iso8601
      }
    end
  end
end

def combine_results(results)
  return nil if results.empty?

  combined = {
    "total" => 0.0,
    "totalData" => 0,
    "successRate_numerator" => 0.0,
    "successRate_denominator" => 0.0,
    "slowest" => 0.0,
    "fastest" => Float::INFINITY,
    "average_total" => 0.0,
    "average_count" => 0,
    "requestsPerSec_total" => 0.0,
    "sizePerRequest_total" => 0,
    "sizePerRequest_count" => 0,
    "sizePerSec_total" => 0.0,
    "p95_latencies" => [],
    "errorDistribution" => Hash.new(0)
  }

  results.each do |res|
    total = res["total"].to_f
    combined["total"] += total

    combined["totalData"] += res["totalData"].to_i

    combined["successRate_numerator"] += total * res["successRate"].to_f
    combined["successRate_denominator"] += total

    combined["slowest"] = [combined["slowest"], res["slowest"].to_f].max
    combined["fastest"] = [combined["fastest"], res["fastest"].to_f].min

    combined["average_total"] += res["average"].to_f * total
    combined["average_count"] += total

    combined["requestsPerSec_total"] += res["requestsPerSec"].to_f
    combined["sizePerRequest_total"] += res["sizePerRequest"].to_f
    combined["sizePerRequest_count"] += 1
    combined["sizePerSec_total"] += res["sizePerSec"].to_f

    combined["p95_latencies"] << res["p95_latency"].to_f

    res["errorDistribution"].each do |err, count|
      combined["errorDistribution"][err] += count.to_i
    end
  end

  {
    "successRate" => (combined["successRate_numerator"] / combined["successRate_denominator"]).round(4),
    "total" => (combined["total"]).round(4),
    "slowest" => (combined["slowest"]).round(4),
    "fastest" => (combined["fastest"] == Float::INFINITY ? 0.0 : combined["fastest"]).round(4),
    "average" => (combined["average_total"] / combined["average_count"]).round(4),
    "requestsPerSec" => (combined["requestsPerSec_total"]).round(4),
    "totalData" => (combined["totalData"]).round(4),
    "sizePerRequest" => (combined["sizePerRequest_total"] / combined["sizePerRequest_count"]).round(4),
    "sizePerSec" => (combined["sizePerSec_total"]).round(4),
    "errorDistribution" => combined["errorDistribution"],
    "p95_latency" => combined["p95_latencies"].sort[(combined["p95_latencies"].size * 0.95).floor] || 0.0
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

def save_result(result, test_name, server_name)
  path_prefix = File.join("results", cpu_label, test_name)
  FileUtils.mkdir_p(path_prefix)
  path = File.join(path_prefix, "#{server_name}.json")
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
      results = []
      next if filters.any?{|filter| !(filter =~ server.name.to_s || filter =~ test_name.to_s) }
      Array(test_case.threads).each do |threads|
        Array(test_case.workers).each do |workers|
          [true, false].each do |http2|
            next unless server.exec_found?
            next unless test_case.requires.all?{|r| server.supports?(r) }
            next if http2 && !server.supports?(:http2)
            next if !http2 && test_case.grpc?

            server_config_file_path = File.join(File.dirname(path), "server_configurations", "#{server.name}.rb")
            server_config_file_path = "server_configurations/#{server.name}.rb" unless File.exist?(server_config_file_path)

            results.concat run_benchmark(test_name, test_case, server_config_file_path, server, threads: threads, workers: workers, http2: http2)
          end
        end
      end

      save_result(results, test_name, server.name.to_s) if results.any?
    end
  rescue StandardError => e
    puts "Error during test case #{path}. #{e}"
    puts e.backtrace
  end
end
