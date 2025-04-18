module Itsi
  class Server
    module Config
      class RequestHeaders < Middleware

        insert_text <<~SNIPPET
        request_headers \\
          additions: { ${1:"X-Correlation-ID" => ["{request_id_full\\}"]} },
          removals:  [${2:"X-Forwarded-For"}]
        SNIPPET

        detail "Add, override or remove request headers before reaching your handler."

        schema do
          {
            additions: Hash(Type(String), Array(Type(String))).default({}),
            removals: Array(Type(String)).default([]),
            combine: Bool().default(true)
          }
        end
      end
    end
  end
end
