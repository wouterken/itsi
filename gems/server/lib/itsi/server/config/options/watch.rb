module Itsi
  class Server
    module Config
      class Watch < Option

        insert_text <<~SNIPPET
        watch "${1:./**/*.rb}", [${2:%w[bundle exec itsi restart]}] # Run a command each time a watched set of files changes
        SNIPPET

        detail "Run a command each time a watched set of files changes"

        def initialize(location, path, commands)
          @path = path
          @commands = commands
          super(location, nil)
        end

        def build!
          path, commands = @path, @commands
          location.instance_eval do
            @options[:notify_watchers] ||= []
            @options[:notify_watchers] << [path, commands]
          end
        end
      end
    end
  end
end
