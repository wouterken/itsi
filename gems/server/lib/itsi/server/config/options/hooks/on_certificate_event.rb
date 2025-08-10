module Itsi
  class Server
    module Config
      class OnCertificateEvent < Option
        include ConfigHelpers

        insert_text <<~SNIPPET
          on_certificate_event do |event|
            case event[:type]
            when :issued
              # Certificate was successfully issued
              puts "Certificate issued for \#{event[:domains].join(', ')}"
            when :renewed
              # Certificate was successfully renewed
              puts "Certificate renewed for \#{event[:domains].join(', ')}"
            when :error
              # Certificate operation failed
              puts "Certificate error for \#{event[:domains].join(', ')}: \#{event[:error]}"
            end
          end
        SNIPPET

        detail "Hook called when certificate lifecycle events occur (issued, renewed, error, etc.)"

        schema do
          (Type(Proc) & Required())
        end

        def initialize(location, &block)
          @location = location
          @block = block
        end

        def build!
          hooks = (@location.options[:hooks] ||= {})
          certificate_hooks = (hooks[:on_certificate_event] ||= [])

          return unless @block

          if certificate_hooks.is_a?(Array)
            certificate_hooks << @block
          else
            hooks[:on_certificate_event] = [certificate_hooks, @block]
          end
        end
      end
    end
  end
end
