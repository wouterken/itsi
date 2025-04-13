module Itsi
  class Server
    module Config
      class Cors < Middleware

        insert_text <<~SNIPPET
        cors \\
          allow_origins: ${1|["*"]|},
          allow_methods: ${2|%w[GET POST PUT DELETE]|},
          allow_headers: ${3|%w[Content-Type Authorization]|},
          allow_credentials: ${4|true,false|},
          expose_headers: ${5|[]|},
          max_age: ${6|3600|}
        SNIPPET

        detail "Enables Cross-Origin Resource Sharing (CORS) for the server."

        schema do
          {
            allow_origins: Array(Type(String)).default(["*"]),
            allow_methods: Array(Type(String)).default(["GET", "POST", "PUT", "DELETE"]),
            allow_headers: Array(Type(String)).default(["Content-Type", "Authorization"]),
            allow_credentials: Bool().default(false),
            expose_headers: Array(Type(String)).default([]),
            max_age: Type(Integer).default(3600)
          }
        end

      end
    end
  end
end
