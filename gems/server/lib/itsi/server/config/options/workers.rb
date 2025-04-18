module Itsi
  class Server
    module Config
      class Workers < Option

        insert_text "workers ${1|1,2,Etc.nprocessors|} # Number of worker processes to run"

        detail "Number of worker processes to run"

        schema do
          Range(1..255)
        end

      end
    end
  end
end
