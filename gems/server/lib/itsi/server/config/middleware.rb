module Itsi
  class Server
    module Config
      class Middleware
        include ConfigHelpers

        def build!
          location.middleware[self.class.middleware_name] = @params
        end
      end
    end
  end
end
