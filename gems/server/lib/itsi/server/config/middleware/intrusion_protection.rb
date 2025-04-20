module Itsi
  class Server
    module Config
      class IntrusionProtection < Middleware

        require_relative "rate_limit_store"
        require_relative "token_source"

        insert_text <<~SNIPPET
        intrusion_protection \\
          banned_url_patterns: ${1|KnownPaths.php_php|},
          banned_header_patterns: { "User-Agent" => ${2|%w[sqlmap curl]|} },
          banned_time_seconds: ${3|300,600|},
          store_config: ${4|"in_memory",{redis:{connection_url:"redis://localhost:6379"}}|},
          error_response: ${5|"forbidden",{ code:403\\, plaintext:{inline:"Access Denied"} }|}
        SNIPPET


        detail "Detects and automatically bans clients that attempt to access suspicious URLs"

        schema do
          {
            banned_url_patterns: Array(Type(String)).default([]),
            banned_header_patterns: Hash(Type(String), Array(Type(String))).default({}),
            banned_time_seconds: Type(Float).default(300),
            store_config: (Required() & Or(Enum(["in_memory"]), Type(RateLimitStore))).default("in_memory"),
            error_response: Type(ErrorResponseDef).default("forbidden"),
            combine: Bool().default(true),
            trusted_proxies: (Hash(Type(String), Type(TokenSource)) & Required()).default({})
          }
        end

        def build!
          @params[:banned_url_patterns] = Array(@params[:banned_url_patterns]).flatten.map do |pattern|
            if pattern.is_a?(Regexp)
              pattern.source
            else
              "#{pattern}$"
            end
          end

          @params[:banned_header_patterns].transform_values! do |patterns|
            Array(patterns).flatten.map do |pattern|
              if pattern.is_a?(Regexp)
                pattern.source
              else
                pattern
              end
            end
          end

          if location.middleware[:intrusion_protection]
            location.middleware[:intrusion_protection] = Array(location.middleware[:intrusion_protection]) + [@params]
          else
            location.middleware[:intrusion_protection] = @params
          end
        end
      end
    end
  end
end
