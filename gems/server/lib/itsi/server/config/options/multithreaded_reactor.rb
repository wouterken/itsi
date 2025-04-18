module Itsi
  class Server
    module Config
      class MultithreadedReactor < Option

        insert_text <<~SNIPPET
        multithreaded_reactor ${1|:auto,true,false|}
        SNIPPET

        detail "Configures whether the server should use a multithreaded reactor."

        schema do
          (Or(Bool(), Enum([:auto])) & Required()).default(:auto) # Default 5 seconds
        end

        def initialize(location, value)
          value = nil if value == :auto
          super
        end

      end
    end
  end
end
