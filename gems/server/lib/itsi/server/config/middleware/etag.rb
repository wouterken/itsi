module Itsi
  class Server
    module Config
      class Etag < Middleware

        insert_text <<~SNIPPET
        etag \\
          type: ${1|"strong","weak"|},
          algorithm: ${2|"sha256","md5"|},
          min_body_size: ${3|0,1024|},
          handle_if_none_match: ${4|true,false|}
        SNIPPET

        detail "Enables ETag generation for the server."

        schema do
          {
            type: (Enum(["strong", "weak"]) + Required()).default("strong"),
            algorithm: (Enum(["sha256", "md5"]) + Required()).default("sha256"),
            min_body_size: Range(0...1024 ** 3).default(0),
            handle_if_none_match: Bool().default(true)
          }
        end
      end
    end
  end
end
