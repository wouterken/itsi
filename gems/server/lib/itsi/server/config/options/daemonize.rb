module Itsi
  class Server
    module Config
      class Daemonize < Option

        insert_text <<~SNIPPET
        daemonize ${1|true,false|}
        SNIPPET

        detail "Configures whether the server should run in the background."

        schema do
          (Bool() & Required())
        end

      end
    end
  end
end
