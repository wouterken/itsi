module Itsi
  class Server
    module KnownPaths
      ALL = []
      Dir.glob(File.join(__dir__, 'known_paths', '**', '*.txt')).each do |file|
        method_name = file[/known_paths\/(.*?)\.txt/,1].gsub(/([a-z])([A-Z])/, "\\1_\\2")
          .gsub(/-|\.|\//, "_")
          .gsub(/(^|\/)[0-9]/){|match| "FO"}.downcase.to_sym

        ALL << method_name
        self.define_singleton_method(method_name) do
          File.readlines(file).map do |s|
            s.force_encoding('UTF-8')
            s.valid_encoding? ? s.strip : s.encode('UTF-8', invalid: :replace, undef: :replace, replace: '').strip
          end
        end
      end
    end
  end
end
