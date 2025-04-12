module Itsi
  class Server
    module Config
      class Compress < Middleware

        insert_text <<~SNIPPET
        compress \\
          min_size: ${1|1024 * 1024|},
          algorithms: ${2|%w[zstd gzip deflate br]|},
          compress_streams: ${3|true,false|},
          mime_types: ${4|%w[all],%w[image],%w[text image audio video font]|},
          level: ${5|"fastest","precise","balanced","best"|}
        SNIPPET

        detail "Enable response compression"

        OtherMimeType = TypedStruct.new do
          {
            other: Type(String)
          }
        end

        schema do
          {
            min_size: (Range(0..1024 ** 4) + Required()).default(1024),
            algorithms: (Array(Enum(%w[zstd gzip deflate br])).default(%w[zstd gzip deflate br])),
            compress_streams: (Bool().default(true)),
            mime_types: Array(Or(Enum(%w[text image application audio video font all]), Type(OtherMimeType))).default(%w[all]),
            level: Enum(%w[fastest precise balanced best]).default("fastest")
          }
        end


      end
    end
  end
end
