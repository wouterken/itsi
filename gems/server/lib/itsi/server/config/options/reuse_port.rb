module Itsi
  class Server
    module Config
      class ReusePort < Option

        insert_text <<~SNIPPET
        reuse_port ${1|true,false|}
        SNIPPET

        detail "Configures whether the server should set the reuse_port option on the underlying socket."

        schema do
          (Bool() & Required()).default(true)
        end

      end
    end
  end
end
