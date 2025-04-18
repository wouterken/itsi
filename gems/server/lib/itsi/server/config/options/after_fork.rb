module Itsi
  class Server
    module Config
      class AfterFork < Option
        insert_text <<~SNIPPET
        after_fork do |pid|
          ${1:# code to run after worker forks}
        end
        SNIPPET

        detail "Run code after worker forks"

        schema do
          (Type(Proc) & Required())
        end

        def initialize(location, &params)
          super(location, params)
        end

        def build!
          location.options[:hooks] ||= {}
          location.options[:hooks][:after_fork] = @params
        end
      end
    end
  end
end
