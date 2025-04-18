module Itsi
  class Server
    module Config
      class StreamBody < Option

        insert_text <<~SNIPPET
        stream_body ${1|true,false|}
        SNIPPET

        detail "Configures whether request bodies should be completed buffered *before* they are forwarded to the application."

        schema do
          Or(Bool(), Type(Symbol)).default(false)
        end
      end
    end
  end
end
