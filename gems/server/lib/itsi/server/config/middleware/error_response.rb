module Itsi
  class Server
    module Config
      InlineContentSource = TypedStruct.new do
        {
          inline: Type(String)
        }
      end

      FileContentSource = TypedStruct.new do
        {
          file: Type(String)
        }
      end

      ContentSource = TypedStruct.new do
        Or(Type(InlineContentSource), Type(FileContentSource))
      end

      ErrorResponse = TypedStruct.new do
        {
          code: Type(Integer) & Required(),
          plaintext: Type(ContentSource),
          html: Type(ContentSource),
          json: Type(ContentSource),
          default: Enum(["plaintext", "html", "json"]) & Required()
        }
      end

      ErrorResponseDef = TypedStruct.new do
        Or(Enum(%w[internal_server_error not_found unauthorized forbidden payload_too_large
        too_many_requests bad_gateway service_unavailable gateway_timeout]), Type(ErrorResponse)) & Required()
      end
    end
  end
end
