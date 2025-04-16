module Itsi
  class Server
    module Config
      class Redirect < Middleware

        insert_text <<~SNIPPET
        redirect \\
          to: "${1:https://example.com/new-path}",
          type: ${2|"permanent","temporary","found","moved_permanently"|}
        SNIPPET

        detail "Automatically redirects incoming requests to a new URL based on a string rewrite rule."

        Redirect = TypedStruct.new do
          {
            to: (Required() & Type(String)),
            type: Enum(["permanent", "temporary", "found", "moved_permanently"]).default("moved_permanently")
          }
        end

        schema Redirect
      end
    end
  end
end
