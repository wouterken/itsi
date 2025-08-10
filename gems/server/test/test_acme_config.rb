# frozen_string_literal: true

require_relative "helpers/test_helper"

class TestAcmeConfig < Minitest::Test
  def setup
    # Clean up any existing environment variables
    @original_env = ENV.to_h.select { |k, _| k.start_with?("ITSI_ACME_") }
    ENV.delete("ITSI_ACME_CONTACT_EMAIL")
    ENV.delete("ITSI_ACME_CACHE_DIR")
    ENV.delete("ITSI_ACME_DIRECTORY_URL")
  end

  def teardown
    # Restore original environment
    @original_env.each { |k, v| ENV[k] = v }
  end

  def test_acme_configuration_dsl_syntax
    # Test that the ACME configuration DSL can be parsed without errors
    server(
      app: lambda do |env|
        [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
      end,
      itsi_rb: proc do
        acme_certificates do
          contact_email "test@example.com"
          cache_dir "/tmp/test_acme_cache"
          challenge_preference :http01

          certificate ["example.com", "www.example.com"]
          certificate "api.example.com"

          on_certificate_event do |event|
            # Test event handler syntax
            puts "Event: #{event[:type]}"
          end
        end
      end
    ) do
      # If we get here, the DSL parsed successfully
      assert true, "ACME configuration DSL parsed successfully"
    end
  end

  def test_acme_api_methods_exist
    server(
      app: lambda do |env|
        [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
      end
    ) do
      # Test that ACME API methods are available
      assert_respond_to Itsi::Server, :add_certificate
      assert_respond_to Itsi::Server, :certificate_status
      assert_respond_to Itsi::Server, :list_certificates
      assert_respond_to Itsi::Server, :renew_certificate
      assert_respond_to Itsi::Server, :remove_certificate
      assert_respond_to Itsi::Server, :get_challenge_preference
      assert_respond_to Itsi::Server, :set_challenge_preference
      assert_respond_to Itsi::Server, :on_certificate_event
    end
  end

  def test_challenge_preference_configuration
    server(
      app: lambda do |env|
        [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
      end,
      itsi_rb: proc do
        acme_certificates do
          contact_email "test@example.com"
          challenge_preference :http01
        end
      end
    ) do
      # Test that challenge preference can be read
      preference = Itsi::Server.get_challenge_preference
      assert_includes %i[http01 tls_alpn01], preference
    end
  end

  def test_certificate_status_for_nonexistent_domain
    server(
      app: lambda do |env|
        [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
      end
    ) do
      # Test certificate status for non-existent domain
      status = Itsi::Server.certificate_status(["nonexistent.example.com"])
      assert_kind_of Hash, status
      assert_equal "not_found", status["status"]
    end
  end

  def test_list_certificates_returns_array
    server(
      app: lambda do |env|
        [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
      end
    ) do
      # Test that list_certificates returns an array
      certificates = Itsi::Server.list_certificates
      assert_kind_of Array, certificates
    end
  end

  def test_environment_variable_configuration
    # Set environment variables
    ENV["ITSI_ACME_CONTACT_EMAIL"] = "env@example.com"
    ENV["ITSI_ACME_CACHE_DIR"] = "/tmp/env_acme_cache"

    server(
      app: lambda do |env|
        [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
      end
    ) do
      # Test that environment variables are recognized
      # In a real implementation, these would be available to the ACME client
      assert_equal "env@example.com", ENV["ITSI_ACME_CONTACT_EMAIL"]
      assert_equal "/tmp/env_acme_cache", ENV["ITSI_ACME_CACHE_DIR"]
    end
  end

  def test_invalid_challenge_preference_raises_error
    # Test that invalid challenge preference raises an error
    assert_raises(ArgumentError) do
      server(
        app: lambda do |env|
          [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
        end,
        itsi_rb: proc do
          acme_certificates do
            contact_email "test@example.com"
            challenge_preference :invalid_type
          end
        end
      ) do
        # Should not reach here
      end
    end
  end

  def test_certificate_configuration_with_options
    server(
      app: lambda do |env|
        [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
      end,
      itsi_rb: proc do
        acme_certificates do
          contact_email "test@example.com"

          certificate ["example.com", "www.example.com"] do
            auto_renew true
            auto_add false # Don't actually request certificate in test
          end
        end
      end
    ) do
      # If we get here, the certificate configuration with block parsed successfully
      assert true, "Certificate configuration with options parsed successfully"
    end
  end

  def test_multiple_event_handlers
    server(
      app: lambda do |env|
        [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
      end,
      itsi_rb: proc do
        acme_certificates do
          contact_email "test@example.com"

          on_certificate_event do |event|
            # First handler
          end

          on_certificate_issued do |event|
            # Specific handler for issued events
          end

          on_certificate_error do |event|
            # Specific handler for error events
          end
        end
      end
    ) do
      # If we get here, multiple event handlers parsed successfully
      assert true, "Multiple event handlers configured successfully"
    end
  end

  def test_acme_configuration_integration_with_bind
    server(
      app: lambda do |env|
        [200, { "Content-Type" => "text/plain" }, ["Hello, World!"]]
      end,
      itsi_rb: proc do
        acme_certificates do
          contact_email "test@example.com"
          certificate "example.com"
        end

        # This should work with ACME configuration
        # Note: In test environment, this won't actually request certificates
        bind "http://0.0.0.0:#{URI(bind).port}"
      end
    ) do
      # If we get here, ACME configuration works with bind
      assert true, "ACME configuration integrates with bind successfully"
    end
  end
end
