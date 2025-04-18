module Itsi
  class Server
    module Config
      class ShutdownTimeout < Option

        insert_text <<~SNIPPET
        shutdown_timeout 5.0 # Shutdown Timeout in Seconds
        SNIPPET

        detail "Sets the timeout for graceful shutdown of the server."

        schema do
          (Type(Float) & Required()).default(5.0) # Default 5 seconds
        end

      end
    end
  end
end
