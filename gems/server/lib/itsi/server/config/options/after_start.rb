module Itsi
  class Server
    module Config
      class AfterStart < Option
        insert_text <<~SNIPPET
        after_start do |pid|
          ${1:# code to run after worker starts}
        end
        SNIPPET

        detail "Run code after worker starts"

        schema do
          (Type(Proc) & Required())
        end

        def initialize(location, &params)
          super(location, params)
        end

        def build!
          location.options[:hooks] ||= {}
          location.options[:hooks][:after_start] = @params
        end
      end
    end
  end
end
