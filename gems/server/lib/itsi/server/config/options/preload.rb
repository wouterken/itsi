module Itsi
  class Server
    module Config
      class Preload < Option

        insert_text <<~SNIPPET
        preload ${1|true,false,:preload|} # Preload
        SNIPPET

        detail "Configures whether all apps and middleware are preloaded before forking, just specific gem groups, or nothing. Has no effect if running in single mode."

        schema do
          Or(Bool(), Type(Symbol)).default(false)
        end
      end
    end
  end
end
