
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

  def run!(server_config_file_path, test_case)
    @builder_args = {
      base: "bundle exec #{@name}",
      config: server_config_file_path,
      scheme: test_case.https ? 'https' : 'http',
      host: '0.0.0.0',
      app_path: test_case.app.path,
      workers: test_case.workers,
      threads: test_case.threads,
      **@custom_args.transform_values{ |v| v[test_case] }
    }

    port = free_port
    cmd = cmd_template % @builder_args.to_h.merge(port: port).transform_keys(&:to_sym)
    puts Paint["\nStarting server:", :green, :bold]
    puts Paint[cmd, :green]

    @pid = Process.spawn(cmd, out: '/dev/null', err: '/dev/null', pgroup: true)
    begin
      wait_for_port(port)
      puts Paint["pid: #{@pid}", :yellow]
      result = yield  "http://127.0.0.1:#{port}#{test_case.path}"
    rescue StandardError => e
      puts Paint["Server failed to start: #{e.message}", :red, :bold]

      return false
    end
    result
  ensure
    stop!
  end

  def stop!
    Process.kill('TERM', @pid)
    Process.wait(@pid)
  end
end
