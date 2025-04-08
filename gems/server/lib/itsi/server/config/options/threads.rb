module Itsi
  class Server
    module Config
      class Threads < Option

        attr_accessor :threads

        insert_text "threads ${1|1,2,Etc.nprocessors|}"

        detail "Number of threads to run per worker"

        def initialize(parent, threads=1)
          @parent = parent
          @threads = threads
          raise "Thread count must be a positive integer" unless threads.is_a?(Integer) && threads > 0
        end

        def build!
          workers
        end
      end
    end
  end
end
