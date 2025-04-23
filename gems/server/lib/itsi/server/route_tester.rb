module Itsi
  class Server
    # Utility module for printing Itsi route information
    module RouteTester
      require "set"
      require "strscan"

      def format_mw(mw)
        mw_name, mw_args = mw
        case mw_name
        when "app"
          "\e[33mapp\e[0m(#{mw_args["app_proc"].inspect.split(" ")[1][0...-1]})"
        when "log_requests"
          if mw_args["before"] && mw_args["after"]
            "\e[33mlog_requests\e[0m(before: #{mw_args["before"]["format"][0..6]}..., after: #{mw_args["after"]["format"][0..6]}...)"
          elsif mw_args["before"]
            "\e[33mlog_requests\e[0m(before: #{mw_args["before"]["format"][0..6]}...)"
          elsif mw_args["after"]
            "\e[33mlog_requests\e[0m(before: nil, after: #{mw_args["after"]["format"][0..6]}...)"
          end
        when "compress"
          "\e[33mcompress\e[0m(#{mw_args["algorithms"].join(" ")}, #{mw_args["mime_types"]})"
        when "cors"
          "\e[33mcors\e[0m(#{mw_args["allow_origins"].join(" ")}, #{mw_args["allow_methods"].join(" ")})"
        when "etag"
          "\e[33metag\e[0m(#{mw_args["type"]}/#{mw_args["algorithm"]}, #{mw_args["handle_if_none_match"] ? "if_none_match" : ""})"
        when "cache_control"
          "\e[33mcache_control\e[0m(max_age: #{mw_args["max_age"]}, #{mw_args.select do |_, v|
            v == true
          end.keys.join(", ")})"
        when "redirect"
          "\e[33mredirect\e[0m(to: #{mw_args["to"]}, type: #{mw_args["type"]})"
        when "static_assets"
          "\e[33mstatic_assets\e[0m(path: #{mw_args["root_dir"]})"
        when "auth_api_key"
          "\e[33mauth_api_key\e[0m(keys: #{mw_args["valid_keys"].keys}#{mw_args["credentials_file"] ? ", credentials_file: #{mw_args["credentials_file"]}" : ""})"
        when "auth_basic"
          "\e[33mbasic_auth\e[0m(keys: #{mw_args["realm"]}#{mw_args["credentials_file"] ? ", credentials_file: #{mw_args["credentials_file"]}" : ""})"
        when "auth_jwt"
          "\e[33mjwt_auth\e[0m(#{mw_args["verifiers"].keys.join(",")})"
        when "rate_limit"
          key = mw_args["key"].is_a?(Hash) ? mw_args["key"]["parameter"] : mw_args["key"]
          "\e[33mrate_limit\e[0m(rps: #{mw_args["requests"]}/#{mw_args["seconds"]}, key: #{key})"
        when "allow_list"
          "\e[33mallow_list\e[0m(patterns: #{mw_args["allowed_patterns"].join(", ")})"
        when "deny_list"
          "\e[33mdeny_list\e[0m(patterns: #{mw_args["denied_patterns"].join(", ")})"
        when "csp"
          "\e[33mcsp\e[0m(#{mw_args["policy"].map { |k, v| "#{k}: #{v.join(",")}" }.join(", ")})"
        when "intrusion_protection"
          [mw_args].flatten.map do |mw_args|
            "\e[33mintrusion_protection\e[0m(banned_url_patterns: #{mw_args["banned_url_patterns"]&.length}, banned_header_patterns: #{mw_args["banned_header_patterns"]&.keys&.join(", ")}, #{mw_args["banned_time_seconds"]}s)"
          end.join("\n")
        when "request_headers"
          [mw_args].flatten.map do |mw_args|
            "\e[33mrequest_headers\e[0m(added: #{mw_args["additions"].keys}, removed: #{mw_args["removals"]})"
          end.join("\n")
        when "response_headers"
          [mw_args].flatten.map do |mw_args|
            "\e[33mresponse_headers\e[0m(added: #{mw_args["additions"].keys}, removed: #{mw_args["removals"]})"
          end.join("\n")
        when "static_response"
          "\e[response_headers\e[0m(#{mw_args["code"]} body: #{mw_args["body"][0..10]})"
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
