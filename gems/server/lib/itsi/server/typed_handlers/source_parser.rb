module Itsi
  class Server
    module TypedHandlers
      module SourceParser
        require 'prism'


        def self.extract_expr_from_source_location(proc)
          source_location = proc.source_location
          source_lines = IO.readlines(source_location.first)

          proc_line = source_location.last - 1
          first_line = source_lines[proc_line]

          until first_line =~ /(?:lambda|proc|->|def|.*?do\s*\||.*?\{.*?\|)/ || proc_line.zero?
            proc_line -= 1
            first_line = source_lines[proc_line]
          end
          lines = source_lines[proc_line..]
          lines[0] = lines[0][/(?:lambda|proc|->|def|.*?do\s*\||.*?\{.*?\|).*/]
          src_str = lines.first << "\n"
          intermediate = Prism.parse(src_str)

          lines[1..-1].each do |line|
            break if intermediate.success?
            token_count = 0
            line.split(/(?=\s|;|\)|\})/).each do |token|
              src_str << token
              token_count += 1
              intermediate = Prism.parse(src_str)
              next unless intermediate.success? && token_count > 1
              break
            end
          end

          raise 'Source Extraction Failed' unless intermediate.success?

          src = intermediate.value.statements.body.first.yield_self do |s|
            s.type == :call_node ? s.block : s
          end
          params = src.parameters
          params = params.parameters if params.respond_to?(:parameters)
          requireds = (params&.requireds || []).map(&:name)
          optionals = params&.optionals || []
          keywords =  (params&.keywords || []).map do |kw|
            [kw.name, kw.value.slice.gsub(/^_\./, '$.')]
          end.to_h

          [requireds.length, keywords]
        rescue
          [ proc.parameters.select{|p| p == :req }&.length || 0, {}]
        end
      end
    end
  end
end
