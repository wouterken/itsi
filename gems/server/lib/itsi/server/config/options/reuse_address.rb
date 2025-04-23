module Itsi
  class Server
    module Config
      class ReuseAddress < Option

        insert_text <<~SNIPPET
        reuse_address ${1|true,false|}
        SNIPPET

        detail "Configures whether the server should set the reuse_address option on the underlying socket."

        schema do
          (Bool() & Required()).default(false)
        end

      end
    end
  end
end
