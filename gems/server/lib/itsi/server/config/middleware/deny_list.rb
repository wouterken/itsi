module Itsi
  class Server
    module Config

      require_relative "cidr_to_regex"

      include CidrToRegex

      class DenyList < Middleware
        insert_text <<~SNIPPET
        deny_list \\
          denied_patterns: [${1|"127.0.0.1","127.*", /127\.0\.*/|}],
          error_response: ${2|"forbidden",{ code: 403\\, plaintext: { inline: "Access denied" } }|}
        SNIPPET

        detail "Block any clients whose IP matches one of the given regex patterns."

        schema do
          {
            denied_patterns: Array(Type(String)) & Required(),
            error_response: Type(ErrorResponseDef).default("forbidden")
          }
        end

        def initialize(location, params={})
          params[:denied_patterns] = Array(params[:denied_patterns]).map do |pattern|
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
