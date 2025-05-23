module Itsi
  class Server
    module Config
      class Writev < Option

        insert_text <<~SNIPPET
        writev ${1|true,false|}
        SNIPPET

        detail "Set whether HTTP/1 connections should try to use vectored writes"

        schema do
          (Bool() & Required()).default(false)
        end

      end
    end
  end
end
