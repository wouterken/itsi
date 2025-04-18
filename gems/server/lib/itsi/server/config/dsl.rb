module Itsi
  class Server
    module Config
      class DSL
        require_relative "config_helpers"
        require_relative "option"
        require_relative "middleware"

        attr_reader :parent, :children, :middleware, :controller_class, :routes, :http_methods, :protocols,
                    :hosts, :ports, :extensions, :content_types, :accepts, :options

        def self.evaluate(config = Itsi::Server::Config.config_file_path, &blk)
          config = new(routes: ["/"]) do
            if blk
              instance_exec(&blk)
            else
              code = IO.read(config)
              instance_eval(code, config.to_s, 1)
            end
            location("*"){}
          end
          [config.options, config.errors]
        rescue Exception => e
          [{}, [[e, e.backtrace[0]]]]
        end

        def initialize(
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
          @controller_class = nil

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
            middleware_loaders: [],
            middleware_loader: lambda do
              @options[:middleware_loaders].each(&:call)
              @middleware[:app] ||= {}
              @middleware[:app][:app_proc] = @middleware[:app]&.[](:preloader)&.call || DEFAULT_APP[]
              if errors.any?
                error = errors.first.first
                error.set_backtrace(error.backtrace.drop_while{|r| r =~ /itsi\/server\/config/ })
                raise error
              end
              flatten_routes
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
          rescue => e
            @errors << [e, caller[1]]
          end
        end

        Middleware.subclasses.each do |middleware|
          middleware_name = middleware.middleware_name
          define_method(middleware_name) do |*args, **kwargs, &blk|
            middleware.new(self, *args, **kwargs, &blk).build!
          rescue => e
            @errors << [e, caller[1]]
          end
        end

        def log_level(level)
          raise "Log level must be set at the root" unless @parent.nil?

          @options[:log_level] = level.to_s
        end

        def log_target(target)
          raise "Log target must be set at the root" unless @parent.nil?

          @options[:log_target] = target.to_s
        end

        def log_target_filters(target_filters)
          raise "Log target filters must be set at the root" unless @parent.nil?

          @options[:log_target_filters] = target_filters
        end

        def log_format(target)
          raise "Log format must be set at the root" unless @parent.nil?

          @options[:log_format] = target.to_s
        end

        def get(route=nil, app_proc = nil, nonblocking: false, &blk)
          endpoint(route, [:get], app_proc, nonblocking: nonblocking,  &blk)
        end

        def post(route=nil, app_proc = nil, nonblocking: false, &blk)
          endpoint(route, [:post], app_proc, nonblocking: nonblocking,  &blk)
        end

        def put(route=nil, app_proc = nil, nonblocking: false, &blk)
          endpoint(route, [:put], app_proc, nonblocking: nonblocking,  &blk)
        end

        def delete(route=nil, app_proc = nil, nonblocking: false, &blk)
          endpoint(route, [:delete], app_proc, nonblocking: nonblocking,  &blk)
        end

        def patch(route=nil, app_proc = nil, nonblocking: false, &blk)
          endpoint(route, [:patch], app_proc, nonblocking: nonblocking,  &blk)
        end

        def endpoint(route=nil, methods=[], app_proc = nil, nonblocking: false, &blk)
          raise "You must provide either a block or an explicit handler for the endpoint" if app_proc.nil? && blk.nil?

          app_proc = @controller.method(app_proc).to_proc if app_proc.is_a?(Symbol)

          app_proc ||= blk

          num_required, keywords = Itsi::Server::TypedHandlers::SourceParser.extract_expr_from_source_location(app_proc)
          params_schema = keywords[:params]

          if params_schema && num_required > 1
            raise "Cannot accept multiple required parameters in a single endpoint. A single typed or untyped params argument is supported"
          end
          if num_required > 2
            raise "Cannot accept more than two required parameters in a single endpoint. An can either accept a single request argument, or a request and a params argument (which may be typed or untyped)"
          end
          if num_required == 0
            raise "Cannot accept zero required parameters in a single endpoint. Endpoint must accept a request parameter"
          end

          accepts_params = !params_schema.nil? || num_required > 1

          if accepts_params
            app_proc = Itsi::Server::TypedHandlers.handler_for(app_proc, params_schema)
          end

          if route || http_methods.any?
            # For endpoints, it's usually assumed trailing slash and non-trailing slash behaviour is the same
            route ||= ""
            routes = route == "/" ? ["", "/"] : [route]
            location(*routes, methods: http_methods) do
              @middleware[:app] = { preloader: -> { app_proc }, nonblocking: nonblocking }
            end
          else
            app = { preloader: -> { app_proc }, nonblocking: nonblocking }
            @middleware[:app] = app
            location("*") do
              @middleware[:app] = app
            end
          end
        end

        def grpc(*handlers, reflection: true, nonblocking: false, **, &blk)
          if @middleware[:app] && @middleware[:app][:request_type].to_s != "grpc"
            raise "App has already been set. You can use only one of `run` and `rackup_file` or `grpc` per location"
          end

          grpc_reflection(handlers) if reflection

          handlers.each do |handler|
            location(Regexp.new("#{Regexp.escape(handler.class.service_name)}/(?:#{handler.class.rpc_descs.keys.map(&:to_s).join("|")})")) do
              @middleware[:app] = { preloader: -> { Itsi::Server::GrpcInterface.for(handler) }, request_type: "grpc", nonblocking: nonblocking }
              instance_exec(&blk)
            end
          end
        end

        def grpc_reflection(handlers)
          @grpc_reflected_services ||= []
          @grpc_reflected_services.concat(handlers)

          location(["grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo",
                    "grpc.reflection.v1.ServerReflection/ServerReflectionInfo"]) do
            @middleware[:app] = { preloader: lambda {
              Itsi::Server::GrpcInterface.reflection_for(handlers)
            }, request_type: "grpc" }
          end
        end

        def run(app, sendfile: true, nonblocking: false, path_info: "/")
          app_args = { preloader: -> { Itsi::Server::RackInterface.for(app) }, sendfile: sendfile, base_path: "^(?<base_path>#{paths_from_parent.gsub(/\.\*\)$/, ')')}).*$", path_info: path_info, nonblocking: nonblocking }
          base_path =  "^(?<base_path>#{paths_from_parent.gsub(/\.\*\)$/, ')')}).*$"
          @middleware[:app] = app_args
          location("*") do
            @middleware[:app] = app_args
          end
        end

        def rackup_file(rackup_file, nonblocking: false, sendfile: true, path_info: "/")
          raise "Rackup file #{rackup_file} doesn't exist" unless File.exist?(rackup_file)
          app_args = { preloader: -> { Itsi::Server::RackInterface.for(rackup_file) }, sendfile: sendfile, base_path: "^(?<base_path>#{paths_from_parent.gsub(/\.\*\)$/, ')')}).*$", path_info: path_info, nonblocking: nonblocking }
          @middleware[:app] = app_args
          location("*") do
            @middleware[:app] = app_args
          end
        end

        def include(path)
          code = IO.read("#{path}.rb")
          instance_eval(code, "#{path}.rb", 1)
        end

        def after_fork(&block)
          raise "After fork must be set at the root" unless @parent.nil?

          @options[:hooks] ||= {}
          @options[:hooks][:after_fork] = block
        end

        def before_fork(&block)
          raise "Before fork must be set at the root" unless @parent.nil?

          @options[:hooks] ||= {}
          @options[:hooks][:before_fork] = block
        end

        def after_memory_threshold_reached(&block)
          raise "Before fork must be set at the root" unless @parent.nil?

          @options[:hooks] ||= {}
          @options[:hooks][:after_memory_threshold_reached] = block
        end


        def fiber_scheduler(klass_name = true)
          raise "Fiber scheduler must be set at the root" unless @parent.nil?

          klass_name = "Itsi::Scheduler" if klass_name == true
          @options[:scheduler_class] = klass_name if klass_name
        end

        def scheduler_threads(threads = 1)
          raise "Scheduler threads must be set at the root" unless @parent.nil?

          @options[:scheduler_threads] = threads
        end

        def controller(controller=nil)
          if controller
            @controller = controller
          else
            @controller
          end
        end

        def static_response(**args)
          args[:body] = args[:body].bytes
          @middleware[:static_response] = args
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
              parts = seg.split('/')
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
              if v[:combine]
                merged[k] = ([merged[k] || []] + [v]).flatten
              else
                merged[k] = v
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
