module Itsi
  class Server
    module Config
      class DSL
        require_relative "config_helpers"
        require_relative "option"
        require_relative "middleware"

        attr_reader :parent, :children, :middleware, :controller, :routes, :http_methods, :protocols,
                    :hosts, :ports, :extensions, :content_types, :accepts, :options

        def self.evaluate(config = Itsi::Server::Config.config_file_path, &blk) # rubocop:disable Metrics/MethodLength
          config = new(routes: ["/"]) do
            if blk
              instance_exec(&blk)
            else
              code = IO.read(config)
              instance_eval(code, config.to_s, 1)
            end
            location("*") {}
          end
          [config.options, config.errors]
        rescue Exception => e # rubocop:disable Lint/RescueException
          [{}, [[e, e.backtrace[0]]]]
        end

        def initialize( # rubocop:disable Metrics/AbcSize,Metrics/MethodLength,Metrics/PerceivedComplexity,Metrics/CyclomaticComplexity
          parent = nil,
          routes: [],
          methods: [],
          protocols: [],
          hosts: [],
          ports: [],
          extensions: [],
          content_types: [],
          accepts: [],
          controller: self,
          &block
        )
          @parent           = parent
          @children         = []
          @middleware       = {}

          @controller = controller
          @routes = Array(routes).flatten
          @http_methods = methods.map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @protocols = protocols.map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @hosts = hosts.map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @ports = ports.map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @extensions = extensions.map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @content_types = content_types.map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @accepts = accepts.map { |s| s.is_a?(Regexp) ? s : s.to_s }

          @options = {
            nested_locations: [],
            middleware_loader: lambda do
              @options[:nested_locations].each(&:call)
              @middleware[:app] ||= {}
              @middleware[:app][:app_proc] = @middleware[:app]&.[](:preloader)&.call || DEFAULT_APP[]
              [flatten_routes, Config.errors_to_error_lines(errors)]
            end
          }

          @errors = []
          instance_exec(&block)
        end

        def errors
          @children.map(&:errors).flatten(1) + @errors
        end

        Option.subclasses.each do |option|
          option_name = option.option_name
          define_method(option_name) do |*args, **kwargs, &blk|
            option.new(self, *args, **kwargs, &blk).build!
          rescue Exception => e # rubocop:disable Lint/RescueException
            @errors << [e, caller[1]]
          end
        end

        Middleware.subclasses.each do |middleware|
          middleware_name = middleware.middleware_name
          define_method(middleware_name) do |*args, **kwargs, &blk|
            middleware.new(self, *args, **kwargs, &blk).build!
          rescue Config::Endpoint::InvalidHandlerException => e
            @errors << [e, "#{e.backtrace[0]}:in #{e.message}"]
          rescue Exception => e # rubocop:disable Lint/RescueException
            @errors << [e, caller[1]]
          end
        end

        def grpc_reflection(handlers)
          @grpc_reflected_services ||= []
          @grpc_reflected_services.concat(handlers)

          location("grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo",
                   "grpc.reflection.v1.ServerReflection/ServerReflectionInfo") do
            @middleware[:app] = { preloader: lambda {
              Itsi::Server::GrpcInterface.reflection_for(handlers)
            }, request_type: "grpc" }
          end
        end

        def file_server(**args)
          Itsi.log_info "Note: file_server is an alias for static_assets"
          static_assets(**args)
        end

        def flatten_routes
          result = []
          result.concat(@children.flat_map(&:flatten_routes))
          route_options = paths_from_parent
          if route_options
            result << deep_stringify_keys(
              {
                route: Regexp.new("^#{route_options}/?$"),
                methods: @http_methods.any? ? @http_methods : nil,
                protocols: @protocols.any? ? @protocols : nil,
                hosts: @hosts.any? ? @hosts : nil,
                ports: @ports.any? ? @ports : nil,
                extensions: @extensions.any? ? @extensions : nil,
                content_types: @content_types.any? ? @content_types : nil,
                accepts: @accepts.any? ? @accepts : nil,
                middleware: effective_middleware
              }
            )
          end
          result
        end

        def paths_from_parent
          return nil unless @routes.any?

          route_or_str = @routes.map do |seg|
            case seg
            when Regexp
              seg.source
            else
              parts = seg.split("/")
              parts.map do |part|
                case part
                when /^:([A-Za-z_]\w*)(?:\(([^)]*)\))?$/
                  param_name = Regexp.last_match(1)
                  custom     = Regexp.last_match(2)
                  if custom && !custom.empty?
                    "(?<#{param_name}>#{custom})"
                  else
                    "(?<#{param_name}>[^/]+)"
                  end
                when /\*/
                  part.gsub(/\*/, ".*")
                else
                  Regexp.escape(part)
                end
              end.join("/")
            end
          end.join("|")
          if parent && parent.paths_from_parent && parent.paths_from_parent != "(?:/)"
            "#{parent.paths_from_parent}#{route_or_str != "" ? "(?:#{route_or_str})" : ""}"
          else
            route_or_str = "/#{route_or_str}" unless route_or_str.start_with?("/")
            "(?:#{route_or_str})"
          end
        end

        def effective_middleware
          chain = []
          node = self
          while node
            if node.middleware[:app]&.[](:preloader)
              node.middleware[:app][:app_proc] = node.middleware[:app].delete(:preloader).call
            end
            chain << node
            node = node.parent
          end
          chain.reverse!

          merged = {}

          chain.each do |n|
            n.middleware.each do |k, v|
              merged[k] = if v[:combine]
                            ([merged[k] || []] + [v]).flatten
                          else
                            v
                          end
            end
          end
          deep_stringify_keys(merged)
        end

        def deep_stringify_keys(obj)
          case obj
          when Hash
            obj.transform_keys!(&:to_s)
            obj.transform_values! { |v| deep_stringify_keys(v) }
          when Array
            obj.map { |v| deep_stringify_keys(v) }
          else
            obj
          end
        end
      end
    end
  end
end
