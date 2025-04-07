module Itsi
  class Server
    module Config
      class Option

        def self.option_name(name=self.name)
          @option_name ||= name.split("::").last.gsub(/[a-z][A-Z]/, '_\1').downcase.to_sym
        end

        %i[detail documentation insert_text].each do |attr|
          define_singleton_method(attr) do |value=nil|
            @option_class_attrs ||= {}
            if value
              @option_class_attrs[attr] = value
            else
              @option_class_attrs[attr]
            end
          end
        end

      end
    end
  end
end

Dir[File.expand_path(File.dirname(__FILE__) + "/options/**.rb")].each do |file|
  current = Itsi::Server::Config::Option.subclasses
  require file
  following = Itsi::Server::Config::Option.subclasses
  new_class = (following - current).first

  documentation_file = "#{file[/(.*)\.rb/,1]}.md"
  if File.exist?(documentation_file)
    new_class.documentation IO.read(documentation_file)
  end
end
