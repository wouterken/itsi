module Itsi
  class Server
    module Config
      class AcmeCertificates < Option
        include ConfigHelpers

        insert_text <<~SNIPPET
          acme_certificates do
            contact_email "${1:admin@example.com}"
            cache_dir "${2:/var/cache/itsi/acme}"
            challenge_preference ${3|:http01,:tls_alpn01|}

            certificate ${4|"example.com",["example.com", "www.example.com"]|}
          end
        SNIPPET

        detail "Configure ACME (Let's Encrypt) certificate management for automatic SSL/TLS certificates."

        schema do
          (Type(Proc) & Required())
        end

        def initialize(location, &block)
          @location = location
          @block = block
          @config = AcmeCertificatesConfig.new
        end

        def build!
          if @block
            @config.instance_eval(&@block)
            @config.apply_to_location!(@location)
          end
          @location.options[:acme_certificates] = @config
        end

        class AcmeCertificatesConfig
          attr_accessor :contact_email, :cache_dir, :directory_url, :challenge_preference

          def initialize
            @contact_email = nil
            @cache_dir = nil
            @directory_url = nil
            @challenge_preference = :http01
            @certificates = []
            @event_handlers = []
          end

          # Configure ACME settings
          def contact_email(email)
            @contact_email = email
          end

          def cache_dir(dir)
            @cache_dir = dir
          end

          def directory_url(url)
            @directory_url = url
          end

          def challenge_preference(type)
            unless %i[http01 tls_alpn01].include?(type)
              raise ArgumentError, "Invalid challenge type: #{type}. Must be :http01 or :tls_alpn01"
            end

            @challenge_preference = type
          end

          # Add a certificate configuration
          def certificate(domains, email: nil, &block)
            domains = Array(domains)
            raise ArgumentError, "At least one domain must be specified" if domains.empty?

            cert_config = CertificateConfig.new(domains, email || @contact_email)
            cert_config.instance_eval(&block) if block_given?
            @certificates << cert_config
            cert_config
          end

          # Set up event handlers
          def on_certificate_event(&block)
            @event_handlers << block if block_given?
          end

          def on_certificate_issued(&block)
            on_certificate_event do |event|
              block.call(event) if event[:type] == :issued
            end
          end

          def on_certificate_renewed(&block)
            on_certificate_event do |event|
              block.call(event) if event[:type] == :renewed
            end
          end

          def on_certificate_error(&block)
            on_certificate_event do |event|
              block.call(event) if event[:type] == :error
            end
          end

          # Apply the configuration to the server location
          def apply_to_location!(location)
            # Store configuration in location options for later use
            location.options[:acme_contact_email] = @contact_email if @contact_email
            location.options[:acme_cache_dir] = @cache_dir if @cache_dir
            location.options[:acme_directory_url] = @directory_url if @directory_url
            location.options[:acme_challenge_preference] = @challenge_preference
            location.options[:acme_event_handlers] = @event_handlers
            location.options[:acme_certificate_configs] = @certificates.map(&:to_hash)

            # Add hook to apply configuration when server starts
            hooks = location.options[:hooks] ||= {}
            after_start_hooks = hooks[:after_start] ||= []

            after_start_hook = proc do
              apply_runtime_configuration!
            end

            if after_start_hooks.is_a?(Array)
              after_start_hooks << after_start_hook
            elsif after_start_hooks.respond_to?(:call)
              original_hook = after_start_hooks
              hooks[:after_start] = [original_hook, after_start_hook]
            else
              hooks[:after_start] = [after_start_hook]
            end
          end

          # Apply the runtime configuration when server starts
          def apply_runtime_configuration!
            # Set environment variables based on configuration
            ENV["ITSI_ACME_CONTACT_EMAIL"] = @contact_email if @contact_email
            ENV["ITSI_ACME_CACHE_DIR"] = @cache_dir if @cache_dir
            ENV["ITSI_ACME_DIRECTORY_URL"] = @directory_url if @directory_url

            # Set challenge preference
            begin
              Itsi::Server.set_challenge_preference(@challenge_preference) if @challenge_preference
            rescue StandardError => e
              Itsi.log_warn "Failed to set ACME challenge preference: #{e.message}"
            end

            # Set up event handlers
            @event_handlers.each do |handler|
              Itsi::Server.on_certificate_event(&handler)
            rescue StandardError => e
              Itsi.log_warn "Failed to set up ACME event handler: #{e.message}"
            end

            # Add certificates
            @certificates.each do |cert_config|
              cert_config.apply_runtime! if cert_config.auto_add?
            end
          end

          def certificates
            @certificates.dup
          end

          def event_handlers
            @event_handlers.dup
          end

          def to_hash
            {
              contact_email: @contact_email,
              cache_dir: @cache_dir,
              directory_url: @directory_url,
              challenge_preference: @challenge_preference,
              certificates: @certificates.map(&:to_hash),
              event_handlers: @event_handlers
            }
          end
        end

        class CertificateConfig
          attr_reader :domains, :email
          attr_accessor :auto_renew, :auto_add

          def initialize(domains, email)
            @domains = Array(domains)
            @email = email
            @auto_renew = true
            @auto_add = true
          end

          def auto_renew(enabled = true)
            @auto_renew = enabled
          end

          def auto_add(enabled = true)
            @auto_add = enabled
          end

          def auto_add?
            @auto_add
          end

          def auto_renew?
            @auto_renew
          end

          def apply_runtime!
            return unless @auto_add

            begin
              Itsi::Server.add_certificate(@domains, @email)
              Itsi.log_info "Added ACME certificate for domains: #{@domains.join(", ")}"
            rescue StandardError => e
              Itsi.log_error "Failed to add ACME certificate for domains #{@domains.join(", ")}: #{e.message}"
            end
          end

          def to_hash
            {
              domains: @domains,
              email: @email,
              auto_renew: @auto_renew,
              auto_add: @auto_add
            }
          end
        end
      end
    end
  end
end
