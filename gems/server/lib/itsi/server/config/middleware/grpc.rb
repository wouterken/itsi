module Itsi
  class Server
    module Config
      class Grpc < Middleware
        insert_text [
          <<~SNIPPET,
          grpc ${1:MyServiceImpl.new}, nonblocking: ${2|false,true|} do
            ${3:# nested middleware…}
          end
          SNIPPET

          <<~SNIPPET,
          grpc ${1:MyServiceImpl.new}, nonblocking: ${2|false,true|}
          SNIPPET
        ]

        detail [
          "gRPC service with middleware  (with HTTP/2, compression, reflection and JSON gateway)",
          "gRPC service (with HTTP/2, compression, reflection and JSON gateway)"
        ]

        schema do
          {
            handlers: Array(Type(Object)) & Required(),
            reflection: Bool().default(true),
            nonblocking: Bool().default(false),
            inner_block: Type(Proc)
          }
        end

        def initialize(location, *handlers, reflection: true, nonblocking: false, &block)
          super(location, {
            handlers: handlers,
            reflection: reflection,
            nonblocking: nonblocking,
            inner_block: block
          })
        end

        def build!
          location.grpc_reflection(@params[:handlers]) if @params[:reflection]
          nonblocking = @params[:nonblocking]
          blk = @params[:inner_block]
          @params[:handlers].each do |handler|
            location.location(Regexp.new("#{Regexp.escape(handler.class.service_name)}/(?:#{handler.class.rpc_descs.keys.map(&:to_s).join("|")})")) do
              @middleware[:app] = { preloader: -> { Itsi::Server::GrpcInterface.for(handler) }, request_type: "grpc", nonblocking: nonblocking }
              instance_exec(&blk) if blk
            end
          end
        end
      end
    end
  end
end
