module Itsi
  class Server
    module Config
      module DSL
        attr_reader :parent, :children, :filters, :endpoint_defs, :controller_class

        def self.evaluate(filepath)
          new do
            code = IO.read(filepath)
            instance_eval(code, filepath, 1)
          end.to_options
        end

        def initialize(parent = nil, route_specs = [], &block)
          @parent           = parent
          @children         = []
          @filters          = {}
          @endpoint_defs    = [] # Each is [subpath, *endpoint_args]
          @controller_class = nil
          @options          = {}

          # We'll store our array of route specs (strings or a single Regexp).
          @route_specs = Array(route_specs).flatten

          validate_path_specs!(@route_specs)
          instance_exec(&block)
        end

        def to_options
          @options.merge(
            {
              routes: flatten_routes
            }
          )
        end

        def workers(workers)
          raise "Workers must be set at the root" unless @parent.nil?

          @options[:workers] = [workers.to_i, 1].max
        end

        def threads(threads)
          raise "Threads must be set at the root" unless @parent.nil?

          @options[:threads] = [threads.to_i, 1].max
        end

        def rackup_file(rackup_file)
          raise "Rackup file must be set at the root" unless @parent.nil?
          raise "rackup_file already set" if @options[:rackup_file]
          raise "Cannot provide a rackup_file if app is defined" if @options[:app]

          if rackup_file.is_a?(File) && rackup_file.exist?
            @options[:rackup_file] = file_path
          else
            file_path = rackup_file
            @options[:rackup_file] = file_path if File.exist?(file_path)
          end
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

        def run(app)
          if @parent.nil?
            raise "App already set" if @options[:app]
            raise "Cannot provide an app if rackup_file is defined" if @options[:rackup_file]

            @options[:app] = app
          else
            @filters[:rack_app] = { app: -> { app } }
          end
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

        def location(*route_specs, &block)
          route_specs = route_specs.flatten
          child = OptionsDSL.new(self, route_specs, &block)
          @children << child
        end

        # define endpoints
        def endpoint(subpath, *args)
          raise "`endpoint` must be set inside a location block" if @parent.nil?

          @endpoint_defs << [subpath, *args]
        end

        def controller(klass)
          raise "`controller` must be set inside a location block" if @parent.nil?

          @controller_class = klass
        end

        def auth_basic(**args)
          raise "`auth_basic` must be set inside a location block" if @parent.nil?

          @filters[:auth_basic] = args
        end

        def redirect(**args)
          raise "`redirect` must be set inside a location block" if @parent.nil?

          @filters[:redirect] = args
        end

        def auth_jwt(**args)
          raise "`auth_jwt` must be set inside a location block" if @parent.nil?

          @filters[:auth_jwt] = args
        end

        def auth_api_key(**args)
          raise "`auth_api_key` must be set inside a location block" if @parent.nil?

          @filters[:auth_api_key] = args
        end

        def compress(**args)
          raise "`compress` must be set inside a location block" if @parent.nil?

          @filters[:compress] = args
        end

        def rate_limit(name, **args)
          raise "`rate_limit` must be set inside a location block" if @parent.nil?

          @filters[:rate_limit] = { name: name }.merge(args)
        end

        def cors(**args)
          raise "`cors` must be set inside a location block" if @parent.nil?

          @filters[:cors] = args
        end

        def file_server(**args)
          raise "`file_server` must be set inside a location block" if @parent.nil?

          @filters[:file_server] = args
        end

        def flatten_routes
          child_routes = @children.flat_map(&:flatten_routes)
          base_expansions = combined_paths_from_parent
          endpoint_routes = @endpoint_defs.map do |(endpoint_subpath, *endpoint_args)|
            ep_expansions = expand_single_subpath(endpoint_subpath)
            final_regex_str = or_pattern_for(cartesian_combine(base_expansions, ep_expansions))

            {
              route: Regexp.new("^#{final_regex_str}$"),
              filters: effective_filters_with_endpoint(endpoint_args)
            }
          end

          location_route = unless @route_specs.empty?
                             pattern_str = or_pattern_for(base_expansions) # the expansions themselves
                             {
                               route: Regexp.new("^#{pattern_str}$"),
                               filters: effective_filters
                             }
                           end

          result = []
          result.concat(child_routes)
          result.concat(endpoint_routes)
          result << location_route if location_route
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
            cartesian_combine(pex, expansions_for(@route_specs))
          else
            expansions_for(@route_specs)
          end
        end

        def combined_paths_from_parent_for_children
          if parent
            pex = parent.combined_paths_from_parent_for_children
            cartesian_combine(pex, expansions_for(@route_specs))
          else
            expansions_for(@route_specs)
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

        def effective_filters
          # gather from root -> self, overriding duplicates
          merged = merge_ancestor_filters
          # turn into array
          merged.map { |k, v| { type: k, parameters: deep_stringify_keys(v) } }
        end

        def effective_filters_with_endpoint(endpoint_args)
          arr = effective_filters
          # endpoint filter last
          ep_filter_params = endpoint_args.dup
          ep_filter_params << @controller_class if @controller_class
          arr << { type: :endpoint, parameters: deep_stringify_keys(ep_filter_params) }
          arr
        end

        def deep_stringify_keys(hash)
          hash.transform_keys!(&:to_s)
          hash.transform_values! { |v| v.is_a?(Hash) ? deep_stringify_keys(v) : v }
        end

        def merge_ancestor_filters
          chain = []
          node = self
          while node
            chain << node
            node = node.parent
          end
          chain.reverse!

          merged = {}
          chain.each do |n|
            n.filters.each do |k, v|
              merged[k] = v
            end
          end
          merged
        end
      end
    end
  end
end
