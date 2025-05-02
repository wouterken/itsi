module Itsi
  class Server
    module Config
      class ListenBacklog < Option

        insert_text <<~SNIPPET
        listen_backlog ${1|1024,2048,4096|}
        SNIPPET

        detail "Specifies the size of the listen backlog for the socket. Larger backlog sizes can improve performance for high-throughput applications by allowing more pending connections to queue, but may increase memory usage. The default value is 1024."

        schema do
          (Type(Integer) & Range(1..Float::INFINITY) & Required()).default(1024)
        end

      end
    end
  end
end
