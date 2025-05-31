module Itsi
  class Server
    module Config
      class PipelineFlush < Option

        insert_text <<~SNIPPET
        pipeline_flush ${1|true,false|}
        SNIPPET

        detail "Aggregates flushes to better support pipelined responses. (HTTP1 only)."

        schema do
          (Bool() & Required()).default(false)
        end

      end
    end
  end
end
