module Itsi
  class Server
    module Config
      class Workers < Option

        attr_accessor :workers

        insert_text "workers ${1|1,2,Etc.nprocessors|}"

        detail "Number of worker processes to run"

        def initialize(parent, workers=1)
          @parent = parent
          @workers = workers
          raise "Worker count must be a positive integer" unless workers.is_a?(Integer) && workers > 0
        end

        def build!
          workers
        end
      end
    end
  end
end
