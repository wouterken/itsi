module Itsi
  class Server
    module Config
      class BeforeShutdown < Option
        insert_text <<~SNIPPET
        before_shutdown do
          ${1:# code to run before worker shuts down}
        end
        SNIPPET

        detail "Run code before worker shuts down"

        schema do
          (Type(Proc) & Required())
        end

        def initialize(location, &params)
          super(location, params)
        end

        def build!
          location.options[:hooks] ||= {}
          location.options[:hooks][:before_shutdown] = @params
        end
      end
    end
  end
end
