module Itsi
  class Server
    module Config
      class RequestTimeout < Option

        insert_text <<~SNIPPET
        request_timeout 300.0 # Request Timeout in Seconds
        SNIPPET

        detail "Set Request Timeout"

        schema do
          (Type(Float) & Required()).default(300.0) # Default 5 minutes
        end

      end
    end
  end
end
