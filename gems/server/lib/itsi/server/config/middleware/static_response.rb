module Itsi
  class Server
    module Config
      class StaticResponse < Middleware
        insert_text <<~SNIPPET
        static_response \\
          code: ${1|200,404,500|}, \\
          headers: [${2|["Content-Type","text/plain"],["Cache-Control","max-age=60"]|}], \\
          body: ${3|"OK".bytes, "Not Found".bytes|}
        SNIPPET

        detail "Immediately return a fixed HTTP response with code, headers, and body."

        schema do
          {
            code:    (Type(Integer) & Required()),
            headers: Array(Array(Type(String), Type(String))).default([]),
            body:    Type(String).default("")
          }
        end

        def initialize(location, params)
          super
          @params[:body] = @params[:body].bytes
        end
      end
    end
  end
end
