module Itsi
  class Server
    module Config
      class AuthBasic < Middleware

        insert_text <<~SNIPPET
        auth_basic \\
          realm: ${1:"Admin Area"},
          credential_pairs: ${2|{ "admin": ENV['ADMIN_PASSWORD'] }|}
        SNIPPET

        detail "Require Basic Auth"

        schema do
          {
            credential_pairs: Hash(Type(String), Type(String)),
            credentials_file: Type(String),
            realm: (Type(String) & Required()).default("Admin Area")
          }
        end

        def initialize(location, params={})
          super
          unless @params[:credential_pairs]&.any?
            if File.exist?(".itsi-credentials") && !@params[:credential_file]
              @params[:credential_file] = ".itsi-credentials"
            end

            if @params[:credential_file] && File.exist?(@params[:credential_file])
              @params[:credential_pairs] = Passfile.load(@params[:credential_file])
            end
          end

          @params[:credential_pairs].compact!

          unless @params[:credential_pairs]&.any?
            raise "No credentials provided"
          end
        end

      end
    end
  end
end
