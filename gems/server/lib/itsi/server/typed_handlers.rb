require_relative "typed_handlers/source_parser"
require_relative "typed_handlers/param_parser"

module Itsi
  class Server
    module TypedHandlers
      def self.handler_for(proc, input_schema)
        input_schema = proc.binding.eval(input_schema) if input_schema
        lambda do |req|
          req.params(input_schema) do |params|
            proc.call(req, params: params)
          end
        end
      end
    end
  end
end
