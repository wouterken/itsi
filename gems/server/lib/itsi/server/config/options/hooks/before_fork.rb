module Itsi
  class Server
    module Config
      class BeforeFork < Option
        insert_text <<~SNIPPET
        before_fork do
          ${1:# code to run before worker forks}
        end
        SNIPPET

        detail "Run code before worker forks"

        schema do
          (Type(Proc) & Required())
        end

        def initialize(location, &params)
          super(location, params)
        end

        def build!
          location.options[:hooks] ||= {}
          location.options[:hooks][:before_fork] = @params
        end
      end
    end
  end
end
