module Itsi
  class Server
    module Config
      class DSL
        attr_reader :parent, :children, :middleware, :endpoint_defs, :controller_class, :routes, :methods, :protocols,
                    :hosts, :ports, :extensions

        def self.evaluate(filepath = Itsi::Server::Config.config_file_path)
          new do
            code = IO.read(filepath)
            instance_eval(code, filepath, 1)
          end.to_options
        end

        def initialize(parent = nil, routes: [], methods: [], protocols: [], hosts: [], ports: [], extensions: [],
                       controller: self, &block)
          @parent           = parent
          @children         = []
          @middleware       = {}
          @controller_class = nil
          @options          = { middleware_loader: lambda {
            @options[:middleware_loaders].each(&:call)
            flatten_routes
          }, middleware_loaders: [] }
          @controller = controller
          # We'll store our array of route specs (strings or a single Regexp).
          @routes = Array(routes).flatten
          @methods = methods.map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @protocols = protocols.map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @hosts = hosts.map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @ports = ports.map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @extensions = extensions.map { |s| s.is_a?(Regexp) ? s : s.to_s }

          validate_path_specs!(@routes)
          instance_exec(&block)
        end

        def to_options
          @options
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

          ENV["ITSI_LOG"] = level.to_s
        end

        def log_format(format)
          raise "Log format must be set at the root" unless @parent.nil?

          case format.to_s
          when "auto" then nil
          when "ansi" then ENV["ITSI_LOG_ANSI"] = "true"
          when "json", "plain" then ENV["ITSI_LOG_PLAIN"] = "true"
          else raise "Invalid log format '#{format}'"
          end
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

        def options(route, app_proc = nil, &blk)
          endpoint(route, :options, app_proc, &blk)
        end

        def endpoint(route, method, app_proc = nil, &blk)
          raise "`endpoint` must be set inside a location block" if @parent.nil?

          raise "You can't use both a block and an explicit handler in the same endpoint" if blk && app_proc
          raise "You must provide either a block or an explicit handler for the endpoint" if app_proc.nil? && blk.nil?

          app_proc = @controller.method(app_proc).to_proc if app_proc.is_a?(Symbol)

          app_proc ||= blk

          location(route, methods: [method]) do
            @middleware[:app] = { app_proc: app_proc }
          end
        end

        def run(app)
          if @options[:app_loader]
            raise "App has already been set. You can use only one of `run` and `rackup_file` per location"
          end

          if @parent.nil?
            @options[:app_loader] = -> { { "app_proc" => Itsi::Server::RackInterface.for(app) } }
          else
            @middleware[:app] = { app_proc: Itsi::Server::RackInterface.for(app) }
          end
        end

        def rackup_file(rackup_file)
          if @options[:app_loader]
            raise "App has already been set. You can use only one of `run` and `rackup_file` per location"
          end

          raise "Rackup file #{rackup_file} doesn't exist" unless File.exist?(rackup_file)

          if @parent.nil?
            @options[:app_loader] = -> { { "app_proc" => Itsi::Server::RackInterface.for(rackup_file) } }
          else
            @middleware[:app] = { app_proc: Itsi::Server::RackInterface.for(rackup_file) }
          end
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

        def fiber_scheduler(klass_name)
          raise "Fiber scheduler must be set at the root" unless @parent.nil?

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

        def location(*routes, methods: [], protocols: [], hosts: [], ports: [], extensions: [], &block)
          if @parent.nil?
            @options[:middleware_loaders] << lambda {
              routes = routes.flatten
              child = DSL.new(
                self,
                routes: Array(routes),
                methods: Array(methods),
                protocols: Array(protocols),
                hosts: Array(hosts),
                ports: Array(ports),
                extensions: Array(extensions),
                controller: @controller,
                &block
              )
              @children << child
            }
          else
            routes = routes.flatten
            child = DSL.new(self,
                            routes: routes,
                            methods: Array(methods) | self.methods,
                            protocols: Array(protocols) | self.protocols,
                            hosts: Array(hosts) | self.hosts,
                            ports: Array(ports) | self.ports,
                            extensions: Array(extensions) | self.extensions,
                            controller: @controller,
                            &block)
            @children << child
          end
        end

        def controller(controller)
          raise "`controller` must be set inside a location block" if @parent.nil?

          @controller = controller
        end

        def auth_basic(**args)
          raise "`auth_basic` must be set inside a location block" if @parent.nil?

          @middleware[:auth_basic] = args
        end

        def redirect(**args)
          raise "`redirect` must be set inside a location block" if @parent.nil?

          @middleware[:redirect] = args
        end

        def proxy(**args)
          raise "`proxy` must be set inside a location block" if @parent.nil?

          @middleware[:proxy] = args
        end

        def auth_jwt(**args)
          raise "`auth_jwt` must be set inside a location block" if @parent.nil?

          @middleware[:auth_jwt] = args
        end

        def auth_api_key(**args)
          raise "`auth_api_key` must be set inside a location block" if @parent.nil?

          @middleware[:auth_api_key] = args
        end

        def compress(**args)
          raise "`compress` must be set inside a location block" if @parent.nil?

          @middleware[:compression] = args
        end

        def rate_limit(name, **args)
          raise "`rate_limit` must be set inside a location block" if @parent.nil?

          @middleware[:rate_limit] = { name: name }.merge(args)
        end

        def cors(**args)
          raise "`cors` must be set inside a location block" if @parent.nil?

          @middleware[:cors] = args
        end

        def file_server(**args)
          raise "`file_server` must be set inside a location block" if @parent.nil?

          @middleware[:file_server] = args
        end

        def flatten_routes
          child_routes = @children.flat_map(&:flatten_routes)
          base_expansions = combined_paths_from_parent

          location_route = unless @routes.empty?
                             pattern_str = or_pattern_for(base_expansions) # the expansions themselves
                             {
                               route: Regexp.new("^#{pattern_str}$"),
                               methods: @methods.any? ? @methods : nil,
                               protocols: @protocols.any? ? @protocols : nil,
                               hosts: @hosts.any? ? @hosts : nil,
                               ports: @ports.any? ? @ports : nil,
                               extensions: @extensions.any? ? @extensions : nil,
                               middleware: effective_middleware
                             }
                           end

          result = []
          result.concat(child_routes)
          result << deep_stringify_keys(location_route) if location_route
          result
        end

        def validate_path_specs!(specs)
          regexes = specs.select { |s| s.is_a?(Regexp) }
          return unless regexes.size > 1

          raise ArgumentError, "Cannot have multiple raw Regex route specs in a single location."
        end

        # Called by flatten_routes to get expansions from the parent's expansions combined with mine
        def combined_paths_from_parent
          if parent
            pex = parent.combined_paths_from_parent_for_children
            cartesian_combine(pex, expansions_for(@routes))
          else
            expansions_for(@routes)
          end
        end

        def combined_paths_from_parent_for_children
          if parent
            pex = parent.combined_paths_from_parent_for_children
            cartesian_combine(pex, expansions_for(@routes))
          else
            expansions_for(@routes)
          end
        end

        def expand_single_subpath(subpath)
          expansions_for([subpath]) # just treat it as a mini specs array
        end

        def expansions_for(specs)
          return [] if specs.empty?

          if specs.any? { |s| s.is_a? Regexp }
            raise "Cannot combine a raw Regexp with other strings in the same location." if specs.size > 1

            [[:raw_regex, specs.first]]
          else
            specs
          end
        end

        def cartesian_combine(parent_exps, child_exps)
          return child_exps if parent_exps.empty?
          return parent_exps if child_exps.empty?

          if parent_exps.size == 1 && parent_exps.first.is_a?(Array) && parent_exps.first.first == :raw_regex
            raise "Cannot nest under a raw Regexp route."
          end

          if child_exps.size == 1 && child_exps.first.is_a?(Array) && child_exps.first.first == :raw_regex
            raise "Cannot nest a raw Regexp route under a parent string route."
          end

          results = []
          parent_exps.each do |p|
            child_exps.each do |c|
              joined = [p, c].reject(&:empty?).join("/").gsub(%r{/\*/}, "").gsub(%r{//}, "/")
              results << joined
            end
          end
          results
        end

        def or_pattern_for(expansions)
          return "" if expansions.empty?

          if expansions.size == 1 && expansions.first.is_a?(Array) && expansions.first.first == :raw_regex
            raw = expansions.first.last
            return raw.source # Use the raw Regexp's source
          end

          pattern_pieces = expansions.map do |exp|
            if exp.empty?
              "" # => means top-level "/"
            else
              segment_to_regex_with_slash(exp)
            end
          end

          joined = pattern_pieces.join("|")

          "(?:#{joined})"
        end

        def segment_to_regex_with_slash(path_str)
          return "" if path_str == ""

          segments = path_str.split("/")

          converted = segments.map do |seg|
            # :param(...)?
            if seg =~ /^:([A-Za-z_]\w*)(?:\(([^)]*)\))?$/
              param_name = Regexp.last_match(1)
              custom     = Regexp.last_match(2)
              if custom && !custom.empty?
                "(?<#{param_name}>#{custom})"
              else
                "(?<#{param_name}>[^/]+)"
              end
            elsif seg =~ /\*/
              seg.gsub(/\*/, ".*")
            else
              Regexp.escape(seg)
            end
          end

          converted.join("/")
        end

        def effective_middleware
          merged = merge_ancestor_middleware
          merged.map { |k, v| { type: k.to_s, parameters: v } }
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

        def merge_ancestor_middleware
          chain = []
          node = self
          while node
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
      end
    end
  end
end
