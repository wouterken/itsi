# frozen_string_literal: true

require "helpers/test_helper"
require "helpers/local_acme"
require "socket"
require "timeout"

class TestLocalAcmeChallenges < Minitest::Test
  APP = proc { |_env| [200, { "content-type" => "text/plain" }, ["acme-ok"]] }

  class << self
    def local_acme
      raise Minitest::Skip, "Go is required for local ACME tests" unless LocalAcmeAuthority.available?

      return @local_acme if @local_acme

      @local_acme = LocalAcmeAuthority.new
      @local_acme.start
      @previous_env = {}
      @local_acme.env.each do |key, value|
        @previous_env[key] = ENV[key]
        ENV[key] = value
      end
      @local_acme
    end

    def shutdown
      return unless @local_acme

      @previous_env&.each do |key, value|
        value.nil? ? ENV.delete(key) : ENV[key] = value
      end
      @local_acme.stop
    end
  end

  def test_tls_alpn01_issuance_without_http_listener
    domain = "alpn.itsi.test"
    local_acme = self.class.local_acme
    https_bind = "https://0.0.0.0:#{local_acme.tls_port}?cert=acme&domains=#{domain}&acme_email=test@example.com"

    server(app: APP, bind: https_bind) do
      wait_for_https_response(local_acme.tls_port, domain)

      certificate = peer_certificate(local_acme.tls_port, domain)
      assert_certificate_domain(certificate, domain)

      response = https_get(local_acme.tls_port, "/", domain)
      assert_equal "200", response.code
      assert_equal "acme-ok", response.body
    end
  end

  def test_http01_issuance_when_tls_validation_port_is_unavailable
    domain = "http01.itsi.test"
    local_acme = self.class.local_acme
    app_port = free_tcp_port
    https_bind = "https://0.0.0.0:#{app_port}?cert=acme&domains=#{domain}&acme_email=test@example.com"
    http_bind = "http://0.0.0.0:#{local_acme.http_port}"

    server(app: APP, bind: https_bind, binds: [https_bind, http_bind]) do
      wait_for_https_response(app_port, domain)

      certificate = peer_certificate(app_port, domain)
      assert_certificate_domain(certificate, domain)

      response = https_get(app_port, "/", domain)
      assert_equal "200", response.code
      assert_equal "acme-ok", response.body
    end
  end

  def test_dynamic_http01_issuance_with_runtime_domain_registration
    domain = "runtime-http01.itsi.test"
    local_acme = self.class.local_acme
    app_port = free_tcp_port
    https_bind = "https://0.0.0.0:#{app_port}?cert=acme&acme_email=test@example.com"
    http_bind = "http://0.0.0.0:#{local_acme.http_port}"

    server(app: APP, bind: https_bind, binds: [https_bind, http_bind]) do
      assert_equal [], Itsi::Server.tls_domains
      assert_equal ["tcp://0.0.0.0:#{app_port}"], Itsi::Server.tls_bindings

      Itsi::Server.register_tls_domain(domain)
      wait_until { Itsi::Server.tls_domains.include?(domain) }
      wait_for_https_response(app_port, domain)

      statuses = Itsi::Server.tls_domain_statuses
      active = statuses.find { |status| status["domain"] == domain }
      refute_nil active
      assert_equal "active", active["status"]

      certificate = peer_certificate(app_port, domain)
      assert_certificate_domain(certificate, domain)

      Itsi::Server.unregister_tls_domain(domain)
      wait_until { !Itsi::Server.tls_domains.include?(domain) }
    end
  end

  def test_dynamic_tls_alpn01_issuance_with_runtime_domain_registration
    domain = "runtime-alpn.itsi.test"
    local_acme = self.class.local_acme
    https_bind = "https://0.0.0.0:#{local_acme.tls_port}?cert=acme&acme_email=test@example.com"

    server(app: APP, bind: https_bind) do
      assert_equal [], Itsi::Server.tls_domains

      Itsi::Server.register_tls_domain(domain)
      wait_until { Itsi::Server.tls_domains.include?(domain) }
      wait_for_https_response(local_acme.tls_port, domain)

      statuses = Itsi::Server.tls_domain_statuses
      active = statuses.find { |status| status["domain"] == domain }
      refute_nil active
      assert_equal "active", active["status"]

      certificate = peer_certificate(local_acme.tls_port, domain)
      assert_certificate_domain(certificate, domain)
    end
  end

  private

  def free_tcp_port
    server = TCPServer.new("127.0.0.1", 0)
    port = server.addr[1]
    server.close
    port
  end

  def wait_for_https_response(port, host = "127.0.0.1")
    Timeout.timeout(20) do
      loop do
        begin
          response = https_get(port, "/", host)
          return response if response.code == "200"
        rescue StandardError
          sleep 0.1
        end
      end
    end
  end

  def wait_until(timeout: 10)
    Timeout.timeout(timeout) do
      loop do
        return if yield

        sleep 0.05
      end
    end
  end

  def https_get(port, path, host = "127.0.0.1")
    Net::HTTP.start(
      "127.0.0.1",
      port,
      use_ssl: true,
      verify_mode: OpenSSL::SSL::VERIFY_NONE,
      open_timeout: 1,
      read_timeout: 1
    ) do |http|
      request = Net::HTTP::Get.new(path)
      request["Host"] = host
      http.request(request)
    end
  end

  def peer_certificate(port, hostname)
    tcp_socket = TCPSocket.new("127.0.0.1", port)
    ssl_context = OpenSSL::SSL::SSLContext.new
    ssl_context.verify_mode = OpenSSL::SSL::VERIFY_NONE
    ssl_socket = OpenSSL::SSL::SSLSocket.new(tcp_socket, ssl_context)
    ssl_socket.hostname = hostname if ssl_socket.respond_to?(:hostname=)
    ssl_socket.connect
    ssl_socket.peer_cert
  ensure
    ssl_socket&.close
    tcp_socket&.close
  end

  def assert_certificate_domain(certificate, domain)
    alt_names = certificate.extensions
      .select { |extension| extension.oid == "subjectAltName" }
      .flat_map { |extension| extension.value.split(/,\s*/) }

    assert_includes alt_names, "DNS:#{domain}"
  end
end
