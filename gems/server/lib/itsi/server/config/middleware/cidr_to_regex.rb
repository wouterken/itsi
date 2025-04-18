module Itsi
  class Server
    module CidrToRegex
      require 'ipaddr'

      def range_to_regex(range)
        # Convert an IP range to regex by component
        start_ip, end_ip = range.begin, range.end

        start_parts = start_ip.to_s.split('.').map(&:to_i)
        end_parts   = end_ip.to_s.split('.').map(&:to_i)

        build_regex_from_parts(start_parts, end_parts)
      end

      def part_to_range_regex(start_val, end_val)
        return start_val.to_s if start_val == end_val

        ranges = []
        (start_val..end_val).each do |val|
          ranges << val.to_s
        end

        # Group similar patterns for compact regex
        ranges.map! { |v| Regexp.escape(v) }
        "(#{ranges.join('|')})"
      end

      def build_regex_from_parts(start_parts, end_parts)
        # Build regex for each octet
        parts = []
        (0..3).each do |i|
          if start_parts[i] == end_parts[i]
            parts << Regexp.escape(start_parts[i].to_s)
          else
            parts << part_to_range_regex(start_parts[i], end_parts[i])
          end
        end

        /^#{parts.join('\.')}$/
      end

      def cidr_to_regex(cidr)
        ip_range = IPAddr.new(cidr).to_range
        range_to_regex(ip_range)
      end

    end
  end
end
