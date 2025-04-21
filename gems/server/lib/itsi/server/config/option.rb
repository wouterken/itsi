module Itsi
  class Server
    module Config
      class Option
        include ConfigHelpers

        def build!
          location.options[self.class.option_name] = @params
        end
      end
    end
  end
end
