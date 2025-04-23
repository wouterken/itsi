module Itsi
  class Server
    module Config
      class Endpoint < Middleware

        InvalidHandlerException = Class.new(StandardError)

        insert_text [
          <<~SNIPPET,
          endpoint "${1:/path}", :${2:handler}
          SNIPPET
          <<~SNIPPET,
          endpoint "${1:/path}" do |req|
          $2
          end
          SNIPPET
          <<~SNIPPET,
          endpoint "${1:/path}" do |req, params|
          $2
          end
          SNIPPET
        ]

        detail ["A light-weight HTTP endpoint (controller)", "A light-weight HTTP endpoint (inline block)", "A light-weight HTTP endpoint (inline params)"]

        schema do
          {
            paths: Array(Or(Type(String), Type(Regexp))),
            handler: Type(Proc) & Required(),
            http_methods: Array(Type(String)),
            nonblocking: Bool()
          }
        end

        def initialize(location, path="", handler=nil, http_methods: [], nonblocking: false, &handler_proc)
          raise "Can not combine a controller method and inline handler" if handler && handler_proc
          handler_proc = location.controller.method(handler).to_proc if handler.is_a?(Symbol) || handler.is_a?(String)

          super(
            location,
            { paths: Array(path), handler: handler_proc, http_methods: http_methods, nonblocking: nonblocking }
          )

          num_required, keywords = Itsi::Server::TypedHandlers::SourceParser.extract_expr_from_source_location(handler_proc)
          params_schema = keywords[:params]
          response_schema = keywords[:response_format]
          exception = nil
          if params_schema && num_required > 1
            exception = InvalidHandlerException.new("Cannot accept multiple required parameters in a single endpoint. A single typed or untyped params argument is supported")
          end
          if num_required > 2
            exception = InvalidHandlerException.new("Cannot accept more than two required parameters in a single endpoint. An can either accept a single request argument, or a request and a params argument (which may be typed or untyped). You can also use keyword arguments to anchor response types")
          end
          if num_required == 0
            exception = InvalidHandlerException.new("Cannot accept zero required parameters in a single endpoint. Endpoint must accept a request parameter")
          end
          if response_schema && !(handler_proc.binding.eval(response_schema) rescue false)
            exception = InvalidHandlerException.new("Response Schema by name `#{response_schema}` not found.")
          end
          if params_schema && !(handler_proc.binding.eval(params_schema) rescue false)
            exception = InvalidHandlerException.new("Params Schema by name `#{params_schema}` not found.")
          end

          if exception
            exception.set_backtrace([handler_proc.source_location.join(":")] + caller)
            raise exception
          end

          accepts_params = !params_schema.nil? || num_required > 1

          if accepts_params
            @params[:handler] = Itsi::Server::TypedHandlers.handler_for(@params[:handler], params_schema)
          end
        end

        def build!
          params = @params
          app = { preloader: -> { params[:handler] }, nonblocking: @params[:nonblocking] }

          if @params[:paths] == [""] && @params[:http_methods].empty?
            location.middleware[:app] = app
            location.location("*") do
              @middleware[:app] = app
            end
          else
            @params[:paths] << "" if @params[:paths].empty?
            location.location(*@params[:paths], methods: @params[:http_methods]) do
              @middleware[:app] = app
            end
          end
        end
      end
    end
  end
end
