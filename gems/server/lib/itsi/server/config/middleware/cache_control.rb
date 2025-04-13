module Itsi
  class Server
    module Config
      class CacheControl < Middleware

        insert_text <<~SNIPPET
        cache_control \\
          max_age: ${1|3600,7200|},
          s_max_age: ${2|1800,3600|},
          stale_while_revalidate: ${3|30,60|},
          stale_if_error: ${4|60,120|},
          public: ${5|true,false|},
          private: ${6|true,false|},
          no_cache: ${7|true,false|},
          no_store: ${8|true,false|},
          must_revalidate: ${9|true,false|},
          proxy_revalidate: ${10|true,false|},
          immutable: ${11|true,false|},
          vary: [\"${12:Accept-Encoding}\"],
          additional_headers: { \"${13:X-Custom-Header}\" => \"${14:value}\" }
        SNIPPET

        detail "Sets Cache-Control, Expires, Vary and additional HTTP caching headers."

        schema do
          {
            max_age: (Type(Integer)),
            s_max_age: (Type(Integer)),
            stale_while_revalidate: (Type(Integer)),
            stale_if_error: (Type(Integer)),
            public: Bool().default(false),
            private: Bool().default(false),
            no_cache: Bool().default(false),
            no_store: Bool().default(false),
            must_revalidate: Bool().default(false),
            proxy_revalidate: Bool().default(false),
            immutable: Bool().default(false),
            vary: Array(Type(String)).default([]),
            additional_headers: Hash(Type(String), Type(String)).default({}),
          }
        end
      end
    end
  end
end
