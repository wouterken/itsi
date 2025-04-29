module Itsi
  class Server
    module Config
      module ConfigHelpers
        def self.load_and_register(klass)
          config_type = klass.name.split("::").last.downcase.gsub(/([a-z]()[A-Z])/, '\1_\2')

          listing = [
            Dir[File.expand_path(File.dirname(__FILE__) + "/#{config_type}/**/*.rb")],
            Dir[File.expand_path(File.dirname(__FILE__) + "/#{config_type}s/**/*.rb")]
          ].flatten

          listing.each do |file|
            current = klass.subclasses.dup
            require file
            following = klass.subclasses
            new_class = (following - current).first

            documentation_file = "#{file[/(.*)\.rb/, 1]}.md"
            documentation_file = "#{file[%r{(.*)/[^/]+\.rb}, 1]}/_index.md" unless File.exist?(documentation_file)
            unless File.exist?(documentation_file) && new_class
              new_class&.documentation "Documentation not found"
              next
            end

            new_class.documentation IO.read(documentation_file)
                                      .gsub(/^---.*?\n.*?-+/m, "") # Strip frontmatter
                                      .gsub(/^(```.*?)\{.*?\}.*$/, "\\1") # Strip filename from code blocks
                                      .gsub(/^\{\{[^}]+\}\}/, "") # Strip Hugo blocks
          end
        end

        def normalize_keys!(hash, expected = [])
          hash.keys.each do |key|
            value = hash.delete(key)
            key = key.to_s.downcase.to_sym
            hash[key] = value
            raise "Unexpected key: #{key}" unless expected.include?(key)

            expected -= [key]
          end
          raise "Missing required keys: #{expected.join(", ")}" unless expected.empty?

          hash
        end

        def self.included(cls) # rubocop:disable Metrics/PerceivedComplexity,Metrics/AbcSize,Metrics/CyclomaticComplexity,Metrics/MethodLength

          class << cls
            def subclasses
              @subclasses ||= []
            end
          end

          def cls.inherited(base) # rubocop:disable Metrics/MethodLength,Lint/MissingSuper,Metrics/PerceivedComplexity
            self.subclasses << base

            %i[detail documentation insert_text schema].each do |attr|
              base.define_singleton_method(attr) do |value = nil|
                @middleware_class_attrs ||= {}
                if value
                  @middleware_class_attrs[attr] = value
                else
                  @middleware_class_attrs[attr]
                end
              end

              base.define_method(attr) do |_value = nil|
                self.class.send(attr)
              end
            end

            def base.schema(value = nil, &blk)
              @middleware_class_attrs ||= {}
              if blk
                @middleware_class_attrs[:schema] = TypedStruct.new(&blk)
              elsif value
                @middleware_class_attrs[:schema] = value
              else
                @middleware_class_attrs[:schema]
              end
            end
          end

          load_and_register(cls)

          config_type = cls.name.split("::").last.downcase

          cls.define_singleton_method("#{config_type}_name") do |name = self.name|
            @config_name ||= name.split("::").last.gsub(/([a-z])([A-Z])/, '\1_\2').downcase.to_sym
          end
          cls.define_method(:opt_name) { self.class.send("#{config_type}_name") }
          cls.define_method(:location) { @location }
        end

        def initialize(location, params = {})
          if !self.class.ancestors.include?(Middleware) && !location.parent.nil?
            raise "#{opt_name} must be set at the top level"
          end

          @location = location
          @params = case schema
                    when TypedStruct::Validation
                      schema.validate!(params)
                    when Array
                      default, validation = schema
                      params ? validation.validate!(params) : default
                    when nil
                      nil
                    else
                      schema.new(params).to_h
                    end
        end
      end
    end
  end
end
