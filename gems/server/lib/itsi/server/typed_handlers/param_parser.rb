module Itsi
  class Server
    module TypedHandlers
      module ParamParser
        require 'date'

        class ValidationError < StandardError
          attr_reader :errors
          def initialize(errors)
            @errors = errors
            super("Validation failed: #{errors.join('; ')}")
          end
        end

        # Conversion map for primitive/base type conversions.
        CONVERSION_MAP = {
          String    => ->(v){ v.to_s },
          Symbol    => ->(v){ v.to_sym },
          Integer   => ->(v){ Integer(v) },
          Float     => ->(v){ Float(v) },
          :Number   => ->(v){ Float(v) },
          TrueClass => ->(v){
            case v
            when true, 'true', '1', 1 then true
            when false, 'false', '0', 0 then false
            else raise "Cannot cast #{v.inspect} to Boolean"
            end
          },
          FalseClass => ->(v){
            case v
            when true, 'true', '1', 1 then true
            when false, 'false', '0', 0 then false
            else raise "Cannot cast #{v.inspect} to Boolean"
            end
          },
          :Boolean  => ->(v){
            case v
            when true, 'true', '1', 1 then true
            when false, 'false', '0', 0 then false
            else raise "Cannot cast #{v.inspect} to Boolean"
            end
          },
          Date      => ->(v){ Date.parse(v.to_s) },
          Time      => ->(v){ Time.parse(v.to_s) },
          DateTime  => ->(v){ DateTime.parse(v.to_s) }
        }.compare_by_identity

        # Preprocess the schema into fixed keys (as symbols) and regex keys.
        # Memoizes the result based on the schema.
        def processed_schema(schema)
          @@schema_cache ||= {}
          @@schema_cache[schema] ||= begin
            fixed = {}
            regex = []
            required_params = schema[:_required] || []
            schema.each do |k, schema_def|
              expected_type, required = schema_def, required_params.include?(k)
              if k.is_a?(Regexp)
                regex << [k, [expected_type, required]]
              else
                fixed[k.to_sym] = [expected_type, required]
              end
            end
            [fixed, regex]
          end
        end

        # Helper that converts an array of path segments into a string.
        # For example, [:user, "addresses", 0, :street] becomes "user.addresses[0].street".
        def format_path(path)
          result = "".dup
          path.each do |seg|
            if seg.is_a?(Integer)
              result << "[#{seg}]"
            else
              result << (result.empty? ? seg.to_s : ".#{seg}")
            end
          end
          result
        end

        # In-place casts the value at container[key] according to expected_type.
        # On success, updates container[key] and returns nil.
        # On failure, returns an error message string that uses the formatted path.
        def cast_value!(container, key, expected_type, path)
          if expected_type.is_a?(Array)
            # Only allow homogeneous array types.
            return "Only homogeneous array types are supported at #{format_path(path)}" if expected_type.size != 1

            # Expect container[key] to be an Array; process each element in place.
            unless container[key].is_a?(Array)
              return "Expected an Array at #{format_path(path)}, got #{container[key].class}"
            end
            container[key].each_with_index do |_, idx|
              err = cast_value!(container[key], idx, expected_type.first, path + [idx])
              return err if err
            end
            return nil

          elsif expected_type.is_a?(Hash)
            # Nested schema: expect container[key] to be a Hash; process it in place.
            unless container[key].is_a?(Hash)
              return "Expected a Hash at #{format_path(path)}, got #{container[key].class}"
            end
            begin
              apply_schema!(container[key], expected_type, path)
              return nil
            rescue ValidationError => ve
              return ve.errors.join('; ')
            end

          else
            converter = CONVERSION_MAP[expected_type]
            if converter
              begin
                container[key] = converter.call(container[key])
                return nil
              rescue => e
                return "Invalid value for #{expected_type} at #{format_path(path)}: #{container[key].inspect} (#{e.message})"
              end
            end

            # Fallbacks.
            if expected_type == Array
              unless container[key].is_a?(Array)
                return "Expected Array at #{format_path(path)}, got #{container[key].class}"
              end
              return nil
            elsif expected_type == Hash
              unless container[key].is_a?(Hash)
                return "Expected Hash at #{format_path(path)}, got #{container[key].class}"
              end
              return nil
            elsif expected_type == File && container[key].is_a?(Hash) && container[key][:tempfile].is_a?(Tempfile)
              return nil
            else
              return "Unsupported type: #{expected_type.inspect} at #{format_path(path)}"
            end
          end
        end

        # Applies the schema in place to the given params hash.
        # Fixed keys are converted to symbols, and regex-matched keys remain as strings.
        # The current location in the params is tracked as an array of path segments.
        def apply_schema!(params, schema, path = [])
          errors = []
          processed = processed_schema(schema)
          fixed_schema = processed[0]
          regex_schema = processed[1]

          # Process fixed keys.
          fixed_schema.each do |fixed_key, (expected_type, required)|
            new_path = path + [fixed_key]
            if params.key?(fixed_key)
              # Symbol key present.
            elsif params.key?(fixed_key.to_s)
              params[fixed_key] = params.delete(fixed_key.to_s)
            else
              if required
                errors << "Missing required key: #{format_path(new_path)}"
              else
                params[fixed_key] = nil
              end
              next
            end

            err = cast_value!(params, fixed_key, expected_type, new_path)
            errors << err if err
          end

          # Process regex keys (only string keys not already handled as fixed keys).
          params.keys.select { |k| k.is_a?(String) }.each do |key|
            next if fixed_schema.has_key?(key.to_sym) || fixed_schema.has_key?(key)
            regex_schema.each do |regex, (expected_type, _required)|
              if regex.match(key)
                new_path = path + [key]
                err = cast_value!(params, key, expected_type, new_path)
                errors << err if err
                break  # only use the first matching regex
              end
            end
          end

          raise ValidationError.new(errors) unless errors.empty?
          params
        end
      end
    end
  end
end
