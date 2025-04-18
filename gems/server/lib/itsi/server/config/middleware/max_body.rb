module Itsi
  class Server
    module Config
      class MaxBody < Middleware

        insert_text <<~SNIPPET
        max_body limit_bytes: ${1: 10 * 1024 ** 2} # Maximum body size in bytes
        SNIPPET

        detail "Limit request body size."

        schema do
          {
            limit_bytes: (Type(Integer) & Required()).default(10 * 1024 ** 2) # Default 10 MiB
          }
        end

      end
    end
  end
end
