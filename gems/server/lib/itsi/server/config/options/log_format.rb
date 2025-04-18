module Itsi
  class Server
    module Config
      class LogFormat < Option

        insert_text <<~SNIPPET
        log_format :${1|json,plain|}
        SNIPPET

        detail "This option configures the log format for the server (json/plain)."

        schema do
          (Enum([:json, :plain]) & Required()).default(:json)
        end

      end
    end
  end
end
