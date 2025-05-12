require 'sys/proctable'
include Sys

class Server # rubocop:disable Style/Documentation
  attr_reader :name, :cmd_template, :http2, :supports

  ALL = []

  def initialize(name, cmd_template, http2: false, supports: [], **custom_args)
    @name = name
    @cmd_template = cmd_template
    @http2 = http2
    @supports = supports
    @custom_args = custom_args
    ALL << self
  end

  def supports?(feature)
    @supports.include?(feature)
  end

  def run!(server_config_file_path, test_case, threads, workers)
    port = free_port

    @builder_args = {
      base: "bundle exec #{@name}",
      config: server_config_file_path,
      scheme: test_case.https ? 'https' : 'http',
      host: '0.0.0.0',
      app_path: test_case.app&.path,
      workers: workers,
      threads: threads,
      www: test_case.static_files_root,
      port: port
    }

    @builder_args.merge!(@custom_args.transform_values{ |v| v[test_case, @builder_args] })

    cmd = cmd_template % @builder_args.to_h.transform_keys(&:to_sym)
    puts Paint["\nStarting server:", :green, :bold]
    puts Paint[cmd, :green]

    @pid = Process.spawn(
      {"RUBY_YJIT_ENABLE" => "#{test_case.use_yjit}", "PORT" => port.to_s, "THREADS" => threads.to_s},
      cmd,
      out: '/dev/null',
      err: '/dev/null',
      pgroup: true
    )

    begin
      wait_for_port(port)
      puts Paint["pid: #{@pid}", :yellow]
      paths, methods, data = [Array(test_case.path), Array(test_case.method), Array(test_case.data)]
      combinations = [paths, methods, data].map(&:length).max.times.map do |i|
        [
          "http://127.0.0.1:#{port}/#{paths[[i, paths.size - 1].min].gsub(/^\/+/,'')}",
          methods[[i, methods.size - 1].min],
          data[[i, data.size - 1].min],
        ]
      end
      result = yield combinations
    rescue StandardError => e
      puts Paint["Server failed to start: #{e.message}", :red, :bold]

      return false
    end


    return result
  rescue StandardError => e
    binding.b # rubocop:disable Lint/Debugger
  ensure
    stop!
  end

  def exec_found?
    return @exec_found if defined?(@exec_found)

    @exec_found = system("which #{name} > /dev/null") || system("ls #{name}")
  end

  def rss
    @pid ? ProcTable.ps(pid: @pid).rss : nil
  rescue
    nil
  end

  def stop!
    Process.kill('TERM', @pid)
    Process.wait(@pid)
  end

end
