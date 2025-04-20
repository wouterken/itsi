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

        def initialize(location, controller=nil)
          super

          if controller
            location.instance_eval{ @controller = controller}
          end
        end

        def build!
          if !@params
            location.instance_eval{ @controller }
          end
        end

      end
    end
  end
end
