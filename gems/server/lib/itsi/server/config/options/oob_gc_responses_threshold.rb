module Itsi
  class Server
    module Config
      class OobGcResponsesThreshold < Option

        insert_text <<~SNIPPET
        oob_gc_responses_threshold ${1|128,256,512,1024|} # Trigger GC every N gaps in the request queue.
        SNIPPET

        detail "Sets the threshold for the number of request queue pauses, before triggering garbage collection."

        schema do
          Type(Integer) & Required()
        end
      end
    end
  end
end
