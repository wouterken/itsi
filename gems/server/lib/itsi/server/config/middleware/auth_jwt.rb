module Itsi
  class Server
    module Config
      class AuthJwt < Middleware
        require_relative "token_source"

        insert_text <<~SNIPPET
        auth_jwt \\
          token_source: ${1:{header: {name: 'Authorization', prefix: 'Bearer '}}},
          verifiers: ${2:{"HS256": [ENV['JWT_HS_SECRET_1'], ENV['JWT_HS_SECRET_2']]}},
          audiences: ${3:[]},
          subjects: ${4:[]},
          issuers: ${5:[]},
          leeway: ${6:60}
        SNIPPET

        detail "Require Basic Auth"

        schema do
          {
            token_source: (Type(TokenSource) & Required()).default({header: {name: 'Authorization', prefix: 'Bearer '}}),
            verifiers: (Hash(Type(String), Array(Type(String)) & Length(1..1024))) & Required() & Length(1..32),
            audiences: Array(Type(String)),
            subjects: Array(Type(String)),
            issuers: Array(Type(String)),
            leeway: Type(Integer)
          }
        end

        def initialize(location, params)
          super
          @params[:verifiers].transform_keys!{|k| k.to_s.downcase }
        end

      end
    end
  end
end
