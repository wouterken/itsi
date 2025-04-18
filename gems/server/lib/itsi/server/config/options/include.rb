module Itsi
  class Server
    module Config
      class Include < Option

        insert_text "include \"${1|other_file.rb|}\" # Include another file to be loaded within the current configuration"

        detail "Include another file to be loaded within the current configuration"

        schema do
          Type(String)
        end

        def build!
          code = IO.read("#{@params}.rb")
          location.instance_eval(code, "#{@params}.rb", 1)
        end

      end
    end
  end
end
