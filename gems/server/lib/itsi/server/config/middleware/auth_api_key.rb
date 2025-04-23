module Itsi
  class Server
    module Config
      class AuthApiKey < Middleware
        require_relative "token_source"
        require_relative "error_response"

        insert_text <<~SNIPPET
        auth_api_key \\
          token_source: ${1:{header: {name: 'Authorization', prefix: 'Bearer '}}},
          key_id_source: ${2|nil,{header: {name: 'X-API-Key'}}|},
          error_response: ${3|"Unauthorized", "unauthenticated", { code: 408\\, default_format: "html"\\, html: { inline: "<h1>Unauthorized</h1>" } }|},
          credentials_file: ${4|nil, ".itsi-credentials"|},
          valid_keys: ${5|nil, [ENV['API_KEY_1']]|}
        SNIPPET

        detail "Require API Key Auth"

        schema do
          {
            valid_keys: Or(Array(Type(String)), Hash(Type(String), Type(String))),
            credentials_file: Type(String),
            token_source: (Type(TokenSource) & Required()).default({header: { name: 'Authorization', prefix: 'Bearer ' }}),
            key_id_source: Type(TokenSource).default({header: { name: 'X-Api-Key-Id' }}),
            error_response: Type(ErrorResponseDef).default("unauthorized"),
          }
        end

        def initialize(location, params)
          super
          if @params[:valid_keys] && @params[:valid_keys].is_a?(Array)
            @params[:valid_keys] = @params[:valid_keys].each_with_index.map { |key, index| [index, key] }.to_h
            @params[:key_id_source] = nil
          end

          if File.exist?(".itsi-credentials") && !@params[:credentials_file]
            @params[:credentials_file] = ".itsi-credentials"
          end

          if @params[:credentials_file] && File.exist?(@params[:credentials_file])
            @params[:valid_keys] = Passfile.load(@params[:credentials_file])
          end

          unless @params[:valid_keys]&.any?
            raise "No credentials provided"
          end
        end
      end
    end
  end
end
