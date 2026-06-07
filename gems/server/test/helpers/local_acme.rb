# frozen_string_literal: true

require "fileutils"
require "json"
require "net/http"
require "openssl"
require "rbconfig"
require "shellwords"
require "timeout"
require "tmpdir"

class LocalAcmeAuthority
  PEBBLE_VERSION = "v2.10.1"

  attr_reader :directory_url, :cache_dir, :http_port, :tls_port

  def self.available?
    system("go version > /dev/null 2>&1")
  end

  def initialize
    @workspace = Dir.mktmpdir("itsi-local-acme-")
    @bin_dir = File.join(@workspace, "bin")
    @cache_dir = File.join(@workspace, "cache")
    FileUtils.mkdir_p(@bin_dir)
    FileUtils.mkdir_p(@cache_dir)
    @pids = []
  end

  def start
    install_binaries
    resolve_module_paths

    @acme_port = free_port
    @management_port = free_port
    @dns_port = free_port
    @dns_management_port = free_port
    @http_port = free_port
    @tls_port = free_port

    write_pebble_config
    start_challtestsrv
    start_pebble

    @directory_url = "https://localhost:#{@acme_port}/dir"
    wait_for_https(@directory_url, @pebble_api_ca_path)
  end

  def stop
    @pids.reverse_each do |pid|
      begin
        Process.kill("TERM", pid)
      rescue Errno::ESRCH
        next
      end

      begin
        Timeout.timeout(5) { Process.wait(pid) }
      rescue Timeout::Error
        begin
          Process.kill("KILL", pid)
        rescue Errno::ESRCH
          nil
        end
        begin
          Process.wait(pid)
        rescue Errno::ECHILD
          nil
        end
      rescue Errno::ECHILD
        nil
      end
    end
    FileUtils.remove_entry(@workspace) if File.exist?(@workspace)
  end

  def env
    {
      "ITSI_ACME_DIRECTORY_URL" => @directory_url,
      "ITSI_ACME_CA_PEM_PATH" => @pebble_api_ca_path,
      "ITSI_ACME_CACHE_DIR" => @cache_dir
    }
  end

  private

  def install_binaries
    install_go_binary("github.com/letsencrypt/pebble/v2/cmd/pebble", "pebble")
    install_go_binary("github.com/letsencrypt/pebble/v2/cmd/pebble-challtestsrv", "pebble-challtestsrv")
  end

  def install_go_binary(package_name, binary_name)
    destination = File.join(@bin_dir, binary_name)
    return if File.exist?(destination)

    system(
      {
        "GOBIN" => @bin_dir
      },
      "go", "install", "#{package_name}@#{PEBBLE_VERSION}",
      exception: true
    )
  end

  def resolve_module_paths
    module_json = JSON.parse(`go mod download -json github.com/letsencrypt/pebble/v2@#{PEBBLE_VERSION}`)
    @pebble_module_dir = module_json.fetch("Dir")
    @pebble_api_ca_path = File.join(@pebble_module_dir, "test/certs/pebble.minica.pem")
    @pebble_tls_cert_path = File.join(@pebble_module_dir, "test/certs/localhost/cert.pem")
    @pebble_tls_key_path = File.join(@pebble_module_dir, "test/certs/localhost/key.pem")
  end

  def write_pebble_config
    @pebble_config_path = File.join(@workspace, "pebble-config.json")
    File.write(
      @pebble_config_path,
      JSON.pretty_generate(
        {
          pebble: {
            listenAddress: "0.0.0.0:#{@acme_port}",
            managementListenAddress: "0.0.0.0:#{@management_port}",
            certificate: @pebble_tls_cert_path,
            privateKey: @pebble_tls_key_path,
            httpPort: @http_port,
            tlsPort: @tls_port,
            ocspResponderURL: "",
            externalAccountBindingRequired: false,
            domainBlocklist: [],
            retryAfter: {
              authz: 1,
              order: 1
            },
            keyAlgorithm: "ecdsa"
          }
        }
      )
    )
  end

  def start_challtestsrv
    spawn_process(
      [
        File.join(@bin_dir, "pebble-challtestsrv"),
        "-dnsserver", "127.0.0.1:#{@dns_port}",
        "-management", "127.0.0.1:#{@dns_management_port}",
        "-http01", "",
        "-https01", "",
        "-tlsalpn01", "",
        "-defaultIPv4", "127.0.0.1",
        "-defaultIPv6", ""
      ],
      "challtestsrv"
    )
  end

  def start_pebble
    spawn_process(
      [
        File.join(@bin_dir, "pebble"),
        "-config", @pebble_config_path,
        "-dnsserver", "127.0.0.1:#{@dns_port}",
        "-strict=false"
      ],
      "pebble",
      {
        "PEBBLE_VA_NOSLEEP" => "1",
        "PEBBLE_WFE_NONCEREJECT" => "0",
        "PEBBLE_AUTHZREUSE" => "0"
      }
    )
  end

  def spawn_process(command, name, env = {})
    log_path = File.join(@workspace, "#{name}.log")
    log = File.open(log_path, "w")
    pid = Process.spawn(
      env,
      *command,
      out: log,
      err: log
    )
    @pids << pid
  end

  def free_port
    server = TCPServer.new("127.0.0.1", 0)
    port = server.addr[1]
    server.close
    port
  end

  def wait_for_https(url, ca_file)
    uri = URI(url)
    store = OpenSSL::X509::Store.new
    store.add_file(ca_file)

    Timeout.timeout(20) do
      loop do
        begin
          Net::HTTP.start(
            uri.host,
            uri.port,
            use_ssl: true,
            cert_store: store,
            verify_mode: OpenSSL::SSL::VERIFY_PEER,
            open_timeout: 1,
            read_timeout: 1
          ) do |http|
            response = http.get(uri.request_uri)
            return if response.code.to_i < 500
          end
        rescue StandardError
          sleep 0.1
        end
      end
    end
  end
end
