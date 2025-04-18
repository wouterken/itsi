module Itsi
  class Server
    module Config
      class LogLevel < Option

        insert_text <<~SNIPPET
        log_level :${1|trace,debug,info,warn,error|}

        SNIPPET

        detail "This option configures the log level for the server."

        schema do
          (Enum([:trace, :debug, :info, :warn, :error]) & Required()).default(:info)
        end

      end
    end
  end
end
