module Itsi
  class Server
    module KnownPaths
      ALL = []
      Dir.glob(File.join(__dir__, "known_paths", "**", "*.txt")).each do |file|
        method_name = file[%r{known_paths/(.*?)\.txt}, 1].gsub(/([a-z])([A-Z])/, "\\1_\\2")
                                                         .gsub(%r{-|\.|/}, "_")
                                                         .gsub(%r{(^|/)[0-9]}) do |match|
          match.gsub(/\d/) do |digit|
            %w[zero one two three four five six seven eight nine][digit.to_i]
          end
        end.downcase.to_sym

        ALL << method_name
        define_singleton_method(method_name) do
          File.readlines(file).map do |s|
            s.force_encoding("UTF-8")
            s.valid_encoding? ? s.strip : s.encode("UTF-8", invalid: :replace, undef: :replace, replace: "").strip
          end
        end
      end
    end
  end
end
