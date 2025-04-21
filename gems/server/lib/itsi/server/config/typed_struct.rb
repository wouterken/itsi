# frozen_string_literal: true

require "date"
require "time"

module Itsi
  class Server
    module Config
      module TypedStruct
        VALUE = 0
        VALIDATE = 1

        def self.new(defaults = nil, &defaults_blk)
          defaults = TypedStruct.module_eval(&defaults_blk) if defaults_blk
          return defaults unless defaults.is_a?(Hash)

          defaults.transform_values! { _1.is_a?(Validation) ? _1.default(nil) : _1 }
          Struct.new(*defaults.keys, keyword_init: true) do
            define_method(:initialize) do |*input, validate: true, **raw_input|
              raise "─ Invalid input to #{self}: #{input.last}" if input.last && !input.last.is_a?(Hash)

              raw_input.transform_keys! { |k| k.to_s.downcase.to_sym }
              raw_input.merge!(input.pop.transform_keys! { |k| k.to_s.downcase.to_sym }) if input.last.is_a?(Hash)

              excess_keys = raw_input.keys - defaults.keys

              raise "─ Unsupported keys #{excess_keys}" if excess_keys.any?

              initial_values = defaults.each_with_object({}) do |(k, default_config), inputs|
                value = raw_input.key?(k) ? raw_input[k] : default_config[VALUE].dup
                next inputs[k] = value unless validate

                begin
                  inputs[k] = default_config[VALIDATE].validate!(value)
                rescue StandardError => e
                  raise ArgumentError, "─ #{k}: #{e.message}"
                end
              end

              super(initial_values)
              validator = self.class.instance_eval { @validators }
              instance_eval(&validator) if validator
            end

            define_singleton_method(:validate) do |&blk|
              @validators = blk
            end

            defaults.each do |key, (_, validation)|
              define_method(:"#{key}?") do
                !!self[key]
              end
              define_method(:"#{key}=") do |value|
                self[key] = validation.validate!(value)
              end
            end

            define_method(:[]=) do |key, value|
              super(key, defaults[key][VALIDATE].validate!(value))
            end

            def is_typed_struct?
              true
            end

            def merge_config(other)
              self.class.new(members.map do |key|
                value = self[key]
                has_merged_val = (other.is_a?(Hash) ? other.key?(key) : other.member?(key))
                next [key, value] unless has_merged_val

                [
                  key,
                  TypedStruct.typed_struct?(value) ? value.merge_config(other[key]) : other[key]
                ]
              end.to_h)
            end

            def to_h
              super.transform_values { |v| v.respond_to?(:merge_config) ? v.to_h : v }
            end
          end
        end

        def self.typed_struct?(inst)
          inst.respond_to?(:is_typed_struct?) && inst.is_typed_struct?
        end

        class Validation
          attr_reader :name, :validations
          attr_accessor :next

          def initialize(name, validations)
            @name = name.to_s
            @validations = Array(validations)
            @next = nil
          end

          def inspect
            "#{@name}"
          end

          def &(other)
            tail = self
            tail = tail.next while tail.next
            tail.next = other
            self
          end

          def default(value)
            [value, self]
          end

          def validate!(value)
            value = value.to_s if value.is_a?(Symbol)
            @validations.each do |validation|
              case validation
              when Proc
                validation.call(value)
              when Array
                unless !value || validation.include?(value)
                  raise ArgumentError,
                        "─ `#{@name}` validation failed. Invalid #{validation} value: #{value.inspect}"
                end
              when Range
                unless !value || validation.include?(value)
                  raise ArgumentError,
                        "─ `#{@name}` validation failed. Invalid #{validation} value: #{value.inspect}"
                end
              when Regexp
                unless !value || validation.match?(value)
                  raise ArgumentError,
                        "─ `#{@name}` validation failed. Invalid #{validation} value: #{value.inspect}"
                end
              when Validation
                validation.validate!(value)
              when Class
                if value && !value.is_a?(validation)
                  begin
                    value = \
                      if validation.eql?(Time) then Time.parse(value.to_s)
                      elsif validation.eql?(::Date) then Date.parse(value.to_s)
                      elsif validation.eql?(Float) then Float(value)
                      elsif validation.eql?(Integer) then Integer(value)
                      elsif validation.eql?(Proc)
                        raise ArgumentError, "Invalid #{validation} value: #{value.inspect}" unless value.is_a?(Proc)
                      elsif validation.eql?(String) || validation.eql?(Symbol)
                        unless value.is_a?(String) || value.is_a?(Symbol)
                          raise ArgumentError,
                                "Invalid #{validation} value: #{value.inspect}"
                        end

                        if validation.eql?(String)
                          value.to_s
                        elsif validation.eql?(Symbol)
                          value.to_s.to_sym
                        end
                      else
                        validation.new(value)
                      end
                  rescue StandardError => e
                    raise ArgumentError,
                          "─ `#{@name}` Validation Failed. Invalid #{validation.to_s.split("::").last} value: #{value.inspect}. Failure reason: \n  └─ #{e.message}"
                  end
                end
              end
            end
            if self.next
              self.next.validate!(value)
            else
              value
            end
          end
        end

        {
          Bool: Validation.new(:Bool, [[true, false]]),
          Required: Validation.new(:Required, ->(value) { !value.nil? }),
          Or: lambda { |*validations|
            Validation.new(:Or, lambda { |v|
              return true if v.nil?

              errs = []
              validations.each do |validation|
                v = validation.validate!(v)
                return v
              rescue StandardError => e
                errs << e.message
              end
              raise StandardError.new("─ Validation failed (None match:) \n  └#{errs.join("\n  └")}")
            })
          },
          Range: lambda { |input_range|
            Validation.new(:Range, [input_range])
          },
          Length: lambda { |input_length|
            Validation.new(:Length, ->(value) { input_length === value.length })
          },
          Hash: lambda { |key_type, value_type|
            Validation.new(:Hash, lambda { |hash|
              return true if hash.nil?
              raise StandardError.new("Expected hash got #{hash.class}") unless hash.is_a?(Hash)

              hash.map do |k, v|
                [
                  key_type.validate!(k),
                  value_type.validate!(v)
                ]
              end.to_h
            })
          },
          Type: ->(input_type) { Validation.new(:Type, input_type) },
          Enum: ->(allowed_values) { Validation.new(:Enum, [allowed_values.map { |v| v.is_a?(Symbol) ? v.to_s : v }]) },
          Array: lambda { |*value_validations|
            Validation.new(:Array, [::Array, lambda { |value|
              return true unless value

              raise StandardError.new("Expected Array got #{value.class}") unless value.is_a?(Array)

              value&.map! do |v|
                value_validations.all? { v = _1.validate!(v) }
                v
              end
            }])
          }
        }.each do |name, factory|
          if factory.is_a?(Proc)
            define_singleton_method(name, &factory)
            define_singleton_method(name.to_s.downcase, &factory)
          else
            const_set(name, factory)
            define_singleton_method(name, -> { factory.dup })
            define_singleton_method(name.to_s.downcase, -> { factory.dup })
          end
        end
      end
    end
  end
end
