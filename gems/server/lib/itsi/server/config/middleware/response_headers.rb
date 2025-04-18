module Itsi
  class Server
    module Config
      class ResponseHeaders < Middleware

        insert_text <<~SNIPPET
          response_headers \\
            additions: { ${1:"X-Powered-By" => ["Itsi"]} },
            removals:  [${2:"Server"}]
          SNIPPET


        detail "Add, override or remove response headers before sending to the client."

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
