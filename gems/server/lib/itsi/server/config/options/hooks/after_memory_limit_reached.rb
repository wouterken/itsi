module Itsi
  class Server
    module Config
      class AfterMemoryLimitReached < Option
        insert_text <<~SNIPPET
        after_memory_limit_reached do |pid|
          ${1:# code to run when memory threshold is exceeded}
        end
        SNIPPET

        detail "Run code in a worker after its memory usage exceeds the configured threshold."

        schema do
          (Type(Proc) & Required())
        end

        def initialize(location, &params)
          super(location, params)
        end

        def build!
          location.options[:hooks] ||= {}
          location.options[:hooks][:after_memory_threshold_reached] = @params
        end
      end
    end
  end
end
