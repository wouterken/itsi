module Itsi
  class Server
    module RouteTester

      require "set"
      require "strscan"
      def format_mw(mw)
        case mw.first
        when "app"
          "app #{mw.last['app_proc'].inspect.split(' ')[1]}"
        when "log_requests"
          if mw.last['before'] && mw.last['after']
            "log_requests(before: #{mw.last['before']['format'][0..6]}..., after: #{mw.last['after']['format'][0..6]}...)"
          elsif mw.last['before']
            "log_requests(before: #{mw.last['before']['format'][0..6]}...)"
          elsif mw.last['after']
            "log_requests(before: nil, after: #{mw.last['after']['format'][0..6]}...)"
          end
        when "compression"
          "compress(#{mw.last['algorithms'].join(' ')}, #{mw.last['mime_types']})"
        when "cors"
          "cors(#{mw.last['allow_origins'].join(' ')}, #{mw.last['allow_methods'].join(' ')})"
        else
          mw.first
        end
      end

      def print_route(route_str, stack)
        filters = %w[methods ports protocols extensions].map do |key|
          val = stack[key]
          val ? "#{key}: #{Array(val).join(",")}" : nil
        end.compact
        filter_str = filters.any? ? filters.join(", ") : "(none)"

        middlewares = stack["middleware"].to_a

        puts "─" * 76
        puts "\e[32mRoute:\e[0m      \e[33m#{route_str}\e[0m"
        puts "\e[32mConditions:\e[0m \e[34m#{filter_str}\e[0m"
        puts "\e[32mMiddleware:\e[0m • #{format_mw(middlewares.first)}"
        middlewares[1..].each do |mw|
          puts "            • #{format_mw(mw)}"
        end
      end

      def explode_route_pattern(pattern)
        pattern = pattern.gsub(/^\^|\$$/, "")
        pattern = pattern.gsub("\\", "")
        tokens = parse_expression(StringScanner.new(pattern))
        expand_tokens(tokens)
      end

      # Parses the expression into a nested tree of tokens
      def parse_expression(scanner)
        tokens = []
        buffer = ""

        until scanner.eos?
          if scanner.scan(/\(\?:/)
            tokens << buffer unless buffer.empty?
            buffer = ""
            tokens << parse_alternation(scanner)
          elsif scanner.peek(1) == ")"
            scanner.getch # consume ')'
            break
          else
            buffer << scanner.getch
          end
        end

        tokens << buffer unless buffer.empty?
        tokens
      end

      # Parses inside a non-capturing group (?:A|B|C)
      def parse_alternation(scanner)
        options = []
        current = []

        until scanner.eos?
          if scanner.scan(/\(\?:/)
            current << parse_alternation(scanner)
          elsif scanner.peek(1) == ")"
            scanner.getch # consume ')'
            break
          elsif scanner.peek(1) == "|"
            scanner.getch # consume '|'
            options << current
            current = []
          else
            current << scanner.getch
          end
        end

        options << current
        { alt: options }
      end

      def expand_tokens(tokens)
        parts = tokens.map do |token|
          if token.is_a?(String)
            [token]
          elsif token.is_a?(Hash) && token[:alt]
            # Recurse into each branch of the alternation
            token[:alt].map { |branch| expand_tokens(branch) }.flatten
          else
            raise "Unexpected token: #{token.inspect}"
          end
        end

        # Cartesian product of all parts
        parts.inject([""]) do |acc, part|
          acc.product(part).map { |a, b| a + b }
        end
      end
    end
  end
end
