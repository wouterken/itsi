module Itsi
  class Server
    module Config
      class Patch < Middleware

        insert_text [
          <<~SNIPPET,
          patch "${1:/path}", :${2:handler}
          SNIPPET
          <<~SNIPPET,
          patch "${1:/path}" do |req|
          $2
          end
          SNIPPET
          <<~SNIPPET,
          patch "${1:/path}" do |req, params|
          $2
          end
          SNIPPET
        ]

        detail ["A light-weight PATCH endpoint (controller)", "A light-weight PATCH endpoint (inline block)", "A light-weight PATCH endpoint (inline params)"]

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
          location.endpoint(path, handler, http_methods: ["PATCH"], nonblocking: nonblocking, script_name: script_name, &handler_proc)
        end

        def build!
        end
      end
    end
  end
end
