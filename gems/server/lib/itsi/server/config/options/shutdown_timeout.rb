module Itsi
  class Server
    module Config
      class PinWorkerCores < Option

        insert_text <<~SNIPPET
        pin_worker_cores ${1|true,false|} # Configure whether worker processes should attempt to pin to a single CPU core.
        SNIPPET

        detail "Configure whether worker processes should attempt to pin to a single CPU core"

        schema do
          (Bool() & Required()).default(true) # Default 5 seconds
        end

      end
    end
  end
end
