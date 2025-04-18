module Itsi
  class Server
    module Config
      class WorkerMemoryLimit < Option

        insert_text <<~SNIPPET
        worker_memory_limit ${1|256,512,1024,2048|} * 1024 ** 2 # Worker memory limit in bytes
        SNIPPET

        detail "Set the maximum amount of memory a worker process can use before it is terminated."

        schema do
          Type(Integer) & Required()
        end
      end
    end
  end
end
