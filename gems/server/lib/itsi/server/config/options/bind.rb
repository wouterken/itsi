module Itsi
  class Server
    module Config
      class Bind < Option

        insert_text <<~SNIPPET
        bind "${1|http://0.0.0.0:3000,https://0.0.0.0:3000,http://0.0.0.0,https://0.0.0.0,unix:///tmp/itsi.sock,tls:///tmp/itsi.sock,https://0.0.0.0?cert=/path/to/cert.pem&key=/path/to/key.pem,https://0.0.0.0?cert=acme&domains=domain.com&acme_email=user@example.com,https://0.0.0.0:3001?domains=devdomain.com,http://0.0.0.0:9292,http://0.0.0.0:8080,https://0.0.0.0:8443|}"
        SNIPPET

        detail "Bind the server to a specific address and port."

        schema do
          Type(String) & Required()
        end

        def initialize(location, params={})
          super
        end

        def build!
          (@location.options[:binds] ||= []) << @params
        end
      end
    end
  end
end
