module Itsi
  class Server
    module Config
      class Include < Option

        insert_text "include \"${1|other_file|}\" # Include another file to be loaded within the current configuration"

        detail "Include another file to be loaded within the current configuration"

        schema do
          Type(String)
        end

        def build!
          included_file = @params
          location.instance_eval do
            @included ||= []
            @included << included_file

            if @auto_reloading
              if ENV["BUNDLE_BIN_PATH"]
                watch "#{included_file}.rb", [%w[bundle exec itsi restart]]
              else
                watch "#{included_file}.rb", [%w[itsi restart]]
              end
            end
          end

          filename =  File.expand_path("#{included_file}.rb")

          code = IO.read(filename)
          location.instance_eval(code, filename, 1)

        end

      end
    end
  end
end
