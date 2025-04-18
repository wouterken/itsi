module Itsi
  class Server
    module Config
      class Threads < Option

        insert_text "threads ${1|1,2,Etc.nprocessors|} # Number of threads to run per worker"

        detail "Number of threads to run per worker"

        schema do
          Range(1..255)
        end

      end
    end
  end
end
