module Itsi
  class Server
    module Config
      class RecvBufferSize < Option

        insert_text <<~SNIPPET
        recv_buffer_size ${1|262_144,1_048_576|}
        SNIPPET

        detail "Specifies the size of the receive buffer for the socket. Larger buffer sizes can improve performance for high-throughput applications but may increase memory usage. The default value is 262,144 bytes."

        schema do
          (Type(Integer) & Range(1..Float::INFINITY) & Required()).default(262_144)
        end

      end
    end
  end
end
