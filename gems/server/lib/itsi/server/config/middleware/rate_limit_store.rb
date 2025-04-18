module Itsi
  class Server
    module Config

      RateLimitKey = TypedStruct.new do
        {
          parameter: Or(
            Hash(Enum(["header"]), Hash(Enum(["name"]), Type(String))) & Required(),
            Hash(Enum(["query"]), Type(String)) & Required()
          )
        }
      end

      RateLimitStore = TypedStruct.new do
        {
          redis: Type(TypedStruct.new do
            {
              connection_url: Type(String)& Required()
            }
          end) & Required()
        }
      end
    end
  end
end
