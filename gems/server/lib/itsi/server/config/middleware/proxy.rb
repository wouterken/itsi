module Itsi
  class Server
    module Config
      class Proxy < Middleware

        insert_text <<~SNIPPET
        proxy \\
          to: "${1:http://backend.example.com/api{path}{query}}", \\
          backends: [${2:"127.0.0.1:3001", "127.0.0.1:3002"}], \\
          backend_priority: ${3|"round_robin","ordered","random"|}, \\
          headers: { ${4| "X-Forwarded-For" => { rewrite: "{addr}" },|} }, \\
          verify_ssl: ${5|true,false|}, \\
          timeout: ${6|30,60|}, \\
          tls_sni: ${7|true,false|}, \\
          error_response: ${8|"bad_gateway", "service_unavailable", { code: 503\\, default_format: "html"\\, html: { inline: "<h1>Service Unavailable</h1>" } }|}
        SNIPPET

        detail "Forwards incoming requests to a backend server using dynamic URL rewriting. Supports various backend selection strategies and header overriding."

        schema do
          {
            to: Type(String) & Required(),
            backends: Array(Type(String)),
            backend_priority: Enum(["round_robin", "ordered", "random"]).default("round_robin"),
            headers: Hash(Type(String), Type(String)).default({}),
            verify_ssl: Bool().default(true),
            tls_sni: Bool().default(true),
            timeout: Type(Integer).default(30),
            error_response: Type(ErrorResponseDef).default("bad_gateway"),
          }
        end

        def build!
          require 'uri'
          @params[:backends]||=  URI.extract(@params[:to]).map(&URI.method(:parse)).map{|u| "#{u.scheme}://#{u.host}:#{u.port}" }
          super
        end
      end
    end
  end
end
