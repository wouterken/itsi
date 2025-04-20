module Itsi
  class Server
    module Config
      class Controller < Middleware

        insert_text <<~SNIPPET
        controller ${1:MyControllerClass.new}
        SNIPPET

        detail "Sets the controller scope for named endpoints"

        schema do
          Type(Object) & Required()
        end
      end
    end
  end
end
