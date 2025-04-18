module Itsi
  class Server
    module Config
      class HeaderReadTimeout < Option

        insert_text <<~SNIPPET
        header_read_timeout 2.0 # Request Timeout in Seconds
        SNIPPET

        detail "Header read timeout."

        schema do
          (Type(Float) & Required()).default(2.0) # Default 1 second
        end

      end
    end
  end
end
