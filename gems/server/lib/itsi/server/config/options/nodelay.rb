module Itsi
  class Server
    module Config
      class Nodelay < Option

        insert_text <<~SNIPPET
        nodelay ${1|true,false|}
        SNIPPET

        detail "Configures whether the server should set the nodelay option on the underlying socket."

        schema do
          (Bool() & Required()).default(true)
        end

      end
    end
  end
end
