module Itsi
  class Server
    module Config
      class Post < Middleware

        insert_text [
          <<~SNIPPET,
          post "${1:/path}", :${2:handler}
          SNIPPET
          <<~SNIPPET,
          post "${1:/path}" do |req|
          $2
          end
          SNIPPET
          <<~SNIPPET,
          post "${1:/path}" do |req, params|
          $2
          end
          SNIPPET
        ]

        detail ["A light-weight POST endpoint (controller)", "A light-weight POST endpoint (inline block)", "A light-weight POST endpoint (inline params)"]

        schema do
          {
            paths: Array(Or(Type(String), Type(Regexp))),
            handler: Type(Proc) & Required(),
            http_methods: Array(Type(String)),
            script_name: Type(String).default(nil),
            nonblocking: Bool()
          }
        end

        def initialize(location, path="", handler=nil, http_methods: [], nonblocking: false, script_name: nil, &handler_proc) # rubocop:disable Lint/MissingSuper
          location.endpoint(path, handler, http_methods: ["POST"], nonblocking: nonblocking, script_name: script_name, &handler_proc)
        end

        def build!
        end
      end
    end
  end
end
