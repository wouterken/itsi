module Itsi
  class Server
    module Config
      class SchedulerThreads < Option

        insert_text "scheduler_threads ${1|1,2,Etc.nprocessors|} # Number of non-blocking scheduler threads to run per worker"

        detail "Number of non-blocking scheduler threads to run per worker"

        schema do
          Range(1..255)
        end

      end
    end
  end
end
