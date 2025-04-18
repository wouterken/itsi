module Itsi
  class Server
    module Config
      class LogTargetFilters < Option

        insert_text <<~SNIPPET
        log_target_filters ${1|[],%w[middleware=debug middleware::rate_limit=trace]|}
        SNIPPET

        detail "Specifies the fine-grained target filters for logging. E.g.log_target_filters [\"middleware=debug\", \"middleware::rate_limit=trace\"]."

        schema do
          (Type(Array) & Required()).default([])
        end

      end
    end
  end
end
