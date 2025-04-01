module Itsi
  class Server
    module Config
      class DSL
        attr_reader :parent, :children, :middleware, :controller_class, :routes, :methods, :protocols,
                    :hosts, :ports, :extensions, :content_types, :accepts, :options

        def self.evaluate(config = Itsi::Server::Config.config_file_path, &blk)
          new(routes: ["*"]) do
            if blk
              instance_exec(&blk)
            else
              code = IO.read(config)
              instance_eval(code, config.to_s, 1)
            end
          end.options
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
          @methods = methods.map { |s| s.is_a?(Regexp) ? s : s.to_s }
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
              flatten_routes
            end
          }

          instance_exec(&block)
        end

        def workers(workers)
          raise "Workers must be set at the root" unless @parent.nil?

          @options[:workers] = [workers.to_i, 1].max
        end

        def threads(threads)
          raise "Threads must be set at the root" unless @parent.nil?

          @options[:threads] = [threads.to_i, 1].max
        end

        def oob_gc_responses_threshold(threshold)
          raise "OOB GC responses threshold must be set at the root" unless @parent.nil?

          @options[:oob_gc_responses_threshold] = threshold.to_i
        end

        def log_level(level)
          raise "Log level must be set at the root" unless @parent.nil?

          @options[:log_level] = level.to_s
        end

        def log_format(format)
          raise "Log format must be set at the root" unless @parent.nil?

          @options[:log_format] = format.to_s
        end

        def log_target(target)
          raise "Log target must be set at the root" unless @parent.nil?

          @options[:log_target] = target.to_s
        end

        def get(route, app_proc = nil, &blk)
          endpoint(route, :get, app_proc, &blk)
        end

        def post(route, app_proc = nil, &blk)
          endpoint(route, :post, app_proc, &blk)
        end

        def put(route, app_proc = nil, &blk)
          endpoint(route, :put, app_proc, &blk)
        end

        def delete(route, app_proc = nil, &blk)
          endpoint(route, :delete, app_proc, &blk)
        end

        def patch(route, app_proc = nil, &blk)
          endpoint(route, :patch, app_proc, &blk)
        end

        def endpoint(route, method, app_proc = nil, &blk)
          raise "You can't use both a block and an explicit handler in the same endpoint" if blk && app_proc
          raise "You must provide either a block or an explicit handler for the endpoint" if app_proc.nil? && blk.nil?

          app_proc = @controller.method(app_proc).to_proc if app_proc.is_a?(Symbol)

          app_proc ||= blk

          location(route, methods: [method]) do
            @middleware[:app] = { preloader: -> { app_proc } }
          end
        end

        def grpc(*handlers, reflection: true, **, &blk)
          if @middleware[:app] && @middleware[:app][:request_type].to_s != "grpc"
            raise "App has already been set. You can use only one of `run` and `rackup_file` or `grpc` per location"
          end

          grpc_reflection(handlers) if reflection

          handlers.each do |handler|
            location(Regexp.new("#{Regexp.escape(handler.class.service_name)}/(?:#{handler.class.rpc_descs.keys.map(&:to_s).join("|")})")) do
              @middleware[:app] = { preloader: -> { Itsi::Server::GrpcInterface.for(handler) }, request_type: "grpc" }
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

        def run(app, sendfile: true)
          @middleware[:app] = { preloader: -> { Itsi::Server::RackInterface.for(app) }, sendfile: sendfile }
        end

        def rackup_file(rackup_file)
          raise "Rackup file #{rackup_file} doesn't exist" unless File.exist?(rackup_file)

          @middleware[:app] = { preloader: -> { Itsi::Server::RackInterface.for(rackup_file) } }
        end

        def include(path)
          code = IO.read("#{path}.rb")
          instance_eval(code, "#{path}.rb", 1)
        end

        def bind(bind_str)
          raise "Bind must be set at the root" unless @parent.nil?

          @options[:binds] ||= []
          @options[:binds] << bind_str.to_s
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

        def worker_memory_limit(memory_limit)
          raise "Worker memory limit must be set at the root" unless @parent.nil?

          @options[:worker_memory_limit] = memory_limit
        end

        def multithreaded_reactor(multithreaded)
          raise "Multithreaded reactor must be set at the root" unless @parent.nil?

          @options[:multithreaded_reactor] = !!multithreaded
        end

        def auto_reload_config!
          if ENV["BUNDLE_BIN_PATH"]
            watch "Itsi.rb", [%w[bundle exec itsi restart]]
          else
            watch "Itsi.rb", [%w[itsi restart]]
          end
        end

        def watch(path, commands)
          raise "Watch be set at the root" unless @parent.nil?

          @options[:notify_watchers] ||= []
          @options[:notify_watchers] << [path, commands]
        end

        def fiber_scheduler(klass_name = true)
          raise "Fiber scheduler must be set at the root" unless @parent.nil?

          klass_name = "Itsi::Scheduler" if klass_name == true
          @options[:scheduler_class] = klass_name if klass_name
        end

        def preload(preload)
          raise "Preload must be set at the root" unless @parent.nil?

          @options[:preload] = preload
        end

        def shutdown_timeout(shutdown_timeout)
          raise "Shutdown timeout must be set at the root" unless @parent.nil?

          @options[:shutdown_timeout] = shutdown_timeout.to_f
        end

        def script_name(script_name)
          raise "Script name must be set at the root" unless @parent.nil?

          @options[:script_name] = script_name.to_s
        end

        def stream_body(stream_body)
          raise "Stream body must be set at the root" unless @parent.nil?

          @options[:stream_body] = !!stream_body
        end

        def location(*routes, methods: [], protocols: [], hosts: [], ports: [], extensions: [], content_types: [],
                     accepts: [], &block)
          build_child = lambda {
            @children << DSL.new(
              self,
              routes: routes,
              methods: Array(methods) | self.methods,
              protocols: Array(protocols) | self.protocols,
              hosts: Array(hosts) | self.hosts,
              ports: Array(ports) | self.ports,
              extensions: Array(extensions) | self.extensions,
              content_types: Array(content_types) | self.content_types,
              accepts: Array(accepts) | self.accepts,
              controller: @controller,
              &block
            )
          }
          if @parent.nil?
            @options[:middleware_loaders] << build_child
          else
            build_child[]
          end
        end

        def log_requests(**args)
          @middleware[:log_requests] = args
        end

        def allow_list(**args)
          args[:allowed_patterns] = Array(args[:allowed_patterns]).map do |pattern|
            if pattern.is_a?(Regexp)
              pattern.source
            else
              pattern
            end
          end
          @middleware[:allow_list] = args
        end

        def deny_list(**args)
          args[:denied_patterns] = Array(args[:denied_patterns]).map do |pattern|
            if pattern.is_a?(Regexp)
              pattern.source
            else
              pattern
            end
          end
          @middleware[:deny_list] = args
        end

        def controller(controller)
          @controller = controller
        end

        def auth_basic(**args)
          @middleware[:auth_basic] = args
        end

        def redirect(**args)
          @middleware[:redirect] = args
        end

        def proxy(**args)
          @middleware[:proxy] = args
        end

        def auth_jwt(**args)
          @middleware[:auth_jwt] = args
        end

        def auth_api_key(**args)
          @middleware[:auth_api_key] = args
        end

        def compress(**args)
          @middleware[:compression] = args
        end

        def request_headers(**args)
          @middleware[:request_headers] = args
        end

        def response_headers(**args)
          @middleware[:response_headers] = args
        end

        def rate_limit(**args)
          @middleware[:rate_limit] = args
        end

        def cache_control(**args)
          @middleware[:cache_control] = args
        end

        def etag(**args)
          @middleware[:etag] = args
        end

        def intrusion_protection(**args)
          args[:banned_url_patterns] = Array(args[:banned_url_patterns]).map do |pattern|
            if pattern.is_a?(Regexp)
              pattern.source
            else
              pattern
            end
          end
          @middleware[:intrusion_protection] = args
        end

        def cors(**args)
          @middleware[:cors] = args
        end

        def static_assets(**args)
          root_dir = args[:root_dir] || "."

          if !File.exist?(root_dir)
            warn "Warning: static_assets root_dir '#{root_dir}' does not exist!"
          elsif !File.directory?(root_dir)
            warn "Warning: static_assets root_dir '#{root_dir}' is not a directory!"
          end

          args[:relative_path] = true unless args.key?(:relative_path)

          location(/(?<path_suffix>.*)/, extensions: args[:allowed_extensions] || []) do
            @middleware[:static_assets] = args
          end
        end

        def file_server(**args)
          # Forward to static_assets for implementation
          puts "Note: file_server is an alias for static_assets"
          static_assets(**args)
        end

        def flatten_routes
          result = []
          result.concat(@children.flat_map(&:flatten_routes))
          route_options = paths_from_parent
          if route_options
            result << deep_stringify_keys(
              {
                route: Regexp.new("^#{route_options}$"),
                methods: @methods.any? ? @methods : nil,
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
            when /^:([A-Za-z_]\w*)(?:\(([^)]*)\))?$/
              param_name = Regexp.last_match(1)
              custom     = Regexp.last_match(2)
              if custom && !custom.empty?
                "(?<#{param_name}>#{custom})"
              else
                "(?<#{param_name}>[^/]+)"
              end
            when /\*/
              seg.gsub(/\*/, ".*")
            else
              Regexp.escape(seg).gsub(%r{/$}, ".*")
            end
          end.join("|")

          if parent && parent.paths_from_parent && parent.paths_from_parent != "(?:/.*)"
            "#{parent.paths_from_parent}#{route_or_str != "" ? "(?:#{route_or_str})" : ""}"
          else
            route_or_str = "/#{route_or_str}" unless route_or_str.start_with?("/")
            "(?:#{route_or_str})"
          end
        end

        def effective_middleware
          merged = merge_ancestor_middleware
          merged.map { |k, v| { type: k.to_s, parameters: v } }
        end

        def merge_ancestor_middleware
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
              merged[k] = v
            end
          end
          merged
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
