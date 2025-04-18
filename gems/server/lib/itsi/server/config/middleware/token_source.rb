module Itsi
  class Server
    module Config

      HeaderSource = TypedStruct.new do
        {
          name: Type(String) & Required(),
          prefix: Type(String)
        }
      end

      HeaderSourceOuter = TypedStruct.new do
        {
          header: Type(HeaderSource)
        }
      end

      QuerySource = TypedStruct.new do
        {
          query: Type(String) & Required()
        }
      end

      TokenSource = TypedStruct.new do
        Or(
          Type(HeaderSourceOuter),
          Type(QuerySource)
        )
      end
    end
  end
end
