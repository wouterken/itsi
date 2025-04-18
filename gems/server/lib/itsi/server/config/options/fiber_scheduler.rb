module Itsi
  class Server
    module Config
      class FiberScheduler < Option

        insert_text "fiber_scheduler ${1|true,'Itsi::Scheduler'|} # Enable Fiber Scheduler mode"

        detail "Enable Fiber Scheduler mode"

        schema do
          Or(Bool(), (Type(String) & Required()))
        end

        def build!
          @params = "Itsi::Scheduler" if @params == true
          location.options[:scheduler_class] = @params if @params
        end
      end
    end
  end
end
