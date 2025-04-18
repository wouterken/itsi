module Itsi
  class Server
    module Config
      class AllowList < Middleware
        require_relative "error_response"
        require_relative "cidr_to_regex"

        include CidrToRegex

        insert_text <<~SNIPPET
        allow_list \\
          allowed_patterns: [${1|"127.0.0.1","127.*", /127\.0\.*/|}],
          error_response: ${2|"forbidden",{ code: 403\\, plaintext: { inline: "<h1>Forbidden</h1>" } }|}
        SNIPPET

        detail "Allow only clients whose IP matches one of the given regex patterns."

        schema do
          {
            allowed_patterns: Array(Type(String)) & Required(),
            error_response: Type(ErrorResponseDef).default("forbidden")
          }
        end

        def initialize(location, params={})
          params[:allowed_patterns] = Array(params[:allowed_patterns]).map do |pattern|
            if pattern.is_a?(Regexp)
              pattern.source
            elsif pattern =~ /\A\d{1,3}(?:\.\d{1,3}){3}\/\d{1,2}\z/
              cidr_to_regex(pattern).source
            else
              pattern
            end
          end
          super
        end
      end
    end
  end
end
