module Itsi
  class Server
    module RouteTester

      require "set"
      require "strscan"
      require "debug"
      def format_mw(mw)
        case mw.first
        when "app"
          "\e[33mapp\e[0m(#{mw.last['app_proc'].inspect.split(' ')[1]})"
        when "log_requests"
          if mw.last['before'] && mw.last['after']
            "\e[33mlog_requests\e[0m(before: #{mw.last['before']['format'][0..6]}..., after: #{mw.last['after']['format'][0..6]}...)"
          elsif mw.last['before']
            "\e[33mlog_requests\e[0m(before: #{mw.last['before']['format'][0..6]}...)"
          elsif mw.last['after']
            "\e[33mlog_requests\e[0m(before: nil, after: #{mw.last['after']['format'][0..6]}...)"
          end
        when "compress"
          "\e[33mcompress\e[0m(#{mw.last['algorithms'].join(' ')}, #{mw.last['mime_types']})"
        when "cors"
          "\e[33mcors\e[0m(#{mw.last['allow_origins'].join(' ')}, #{mw.last['allow_methods'].join(' ')})"
        when "etag"
          "\e[33metag\e[0m(#{mw.last['type']}/#{mw.last['algorithm']}, #{mw.last['handle_if_none_match'] ? 'if_none_match' : ''})"
        when "cache_control"
          "\e[33mcache_control\e[0m(max_age: #{mw.last['max_age']}, #{mw.last.select{|_,v| v == true }.keys.join(", ")})"
        when "redirect"
          "\e[33mredirect\e[0m(to: #{mw.last['to']}, type: #{mw.last['type']})"
        when "static_assets"
          "\e[33mstatic_assets\e[0m(path: #{mw.last['root_dir']})"
        else
          "\e[33m#{mw.first}\e[0m"
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
