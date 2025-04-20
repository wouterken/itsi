module Itsi
  class Server
    module Config
      class RateLimit < Middleware
        require_relative "rate_limit_store"
        require_relative "token_source"

        insert_text <<~SNIPPET
        rate_limit \\
          requests: ${1|1,5,100|},
          seconds: ${2|1,5,100|},
          key: ${3|"address",{parameter:{header:{name:"X-Forwarded-For"}}}|},
          store_config: ${4|"in_memory",{redis:{connection_url: "redis://localhost:6379"}}|},
          error_response: ${5|"too_many_requests", { code: 429\\, default_format: "html"\\, html: { inline: "<h1>Unauthorized</h1>" } }|}
        SNIPPET

        detail "Automatically limits the number of requests a client can make within a given time period."


        schema do
          {
            requests: Required() & Type(Integer) & Range(1..2**32),
            seconds: Required() & Type(Integer) & Range(1..2**32),
            key: (Required() & Or(Enum(["address"]), Type(RateLimitKey))).default("address"),
            store_config: (Required() & Or(Enum(["in_memory"]), Type(RateLimitStore))).default("in_memory"),
            error_response: Type(ErrorResponseDef).default("too_many_requests"),
            trusted_proxies: (Hash(Type(String), Type(TokenSource)) & Required()).default({})
          }
        end

      end
    end
  end
end
