module Itsi
  class Server
    module Config
      class Put < Middleware

        insert_text [
          <<~SNIPPET,
          put "${1:/path}", :${2:handler}
          SNIPPET
          <<~SNIPPET,
          put "${1:/path}" do |req|
          $2
          end
          SNIPPET
          <<~SNIPPET,
          put "${1:/path}" do |req, params|
          $2
          end
          SNIPPET
        ]

        detail ["A light-weight PUT endpoint (controller)", "A light-weight PUT endpoint (inline block)", "A light-weight PUT endpoint (inline params)"]

        schema do
          {
            paths: Array(Or(Type(String), Type(Regexp))),
            handler: Type(Proc) & Required(),
            http_methods: Array(Type(String)),
            nonblocking: Bool()
          }
        end

        def initialize(location, path="", handler=nil, http_methods: [], nonblocking: false, &handler_proc)
          location.endpoint(path, handler, http_methods: ["PUT"], nonblocking: nonblocking, &handler_proc)
        end

        def build!
        end
      end
    end
  end
end
