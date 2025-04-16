module Itsi
  class Server
    module Config
      class LogRequests < Middleware

        insert_text <<~SNIPPET
        log_requests \\
          before: { level: ${1|"INFO","WARN","ERROR","DEBUG"|} format: ${2|"[{request_id}] {method} {path_and_query} - {addr} "|} },
          after: { level: ${3|"INFO","WARN","ERROR","DEBUG"|}, format: ${4|"[{request_id}] └─ {status} in {response_time}"|} }
        SNIPPET

        detail "Enable logging before or after requests"

        LogRequestConfig = TypedStruct.new do
          {
            level: (Required() & Enum(%w[INFO WARN ERROR DEBUG])).default("INFO"),
            format: Type(String)
          }
        end

        schema do
          {
            before: Type(LogRequestConfig),
            after: Type(LogRequestConfig)
          }
        end

      end
    end
  end
end
