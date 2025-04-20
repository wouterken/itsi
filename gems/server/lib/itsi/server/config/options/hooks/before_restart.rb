module Itsi
  class Server
    module Config
      class BeforeRestart < Option
        insert_text <<~SNIPPET
        before_restart do
          ${1:# code to run before worker restarts}
        end
        SNIPPET

        detail "Run code before worker restarts"

        schema do
          (Type(Proc) & Required())
        end

        def initialize(location, &params)
          super(location, params)
        end

        def build!
          location.options[:hooks] ||= {}
          location.options[:hooks][:before_restart] = @params
        end
      end
    end
  end
end
