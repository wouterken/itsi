module Itsi
  class Server
    module Config
      class RubyThreadRequestBacklogSize < Option

        insert_text <<~SNIPPET
        ruby_thread_request_backlog_size ${1|10,25,50,100|}
        SNIPPET

        detail "The maximum number of requests that can be queued for processing by the Ruby thread."

        schema do
          (Type(Integer)).default(nil)
        end

      end
    end
  end
end
