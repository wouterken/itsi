class RouterDSL
  attr_reader :parent, :children, :filters, :endpoint_defs, :controller_class

  def initialize(parent = nil, route_specs = [], &block)
    @parent           = parent
    @children         = []
    @filters          = {}
    @endpoint_defs    = [] # Each is [subpath, *endpoint_args]
    @controller_class = nil

    # We'll store our array of route specs (strings or a single Regexp).
    @route_specs = Array(route_specs).flatten

    validate_path_specs!(@route_specs)

    instance_eval(&block) if block_given?
  end

  ################################
  # DSL: location
  ################################

  # location can accept multiple strings (and at most one Regexp).
  # E.g.: location '/foo', '/bar/:id(\d+)' do ... end
  def location(*route_specs, &block)
    route_specs = route_specs.flatten
    child = RouterDSL.new(self, route_specs, &block)
    @children << child
  end

  # define endpoints
  def endpoint(subpath, *args)
    @endpoint_defs << [subpath, *args]
  end

  def controller(klass)
    @controller_class = klass
  end

  # define some filters
  def basic_auth(**args)
    @filters[:basic_auth] = args
  end

  # define some filters
  def redirect(**args)
    @filters[:redirect] = args
  end

  def jwt_auth(**args)
    @filters[:jwt_auth] = args
  end

  def api_key_auth(**args)
    @filters[:api_key_auth] = args
  end

  def compress(**args)
    @filters[:compress] = args
  end

  def rate_limit(name, **args)
    @filters[:rate_limit] = { name: name }.merge(args)
  end

  def cors(**args)
    @filters[:cors] = args
  end

  def file_server(**args)
    @filters[:file_server] = args
  end

  ################################
  # Flattening logic
  ################################

  def flatten_routes
    # We produce an array of route-hashes:
    #   { route: Regexp, filters: [ {type:, params:}, ... ] }
    #
    # The order: children first (most specific), then endpoints, then the location route.

    # 1) Flatten children (most specific first)
    child_routes = @children.flat_map(&:flatten_routes)

    # 2) Build the expansions for the "parent portion" (this location) → we'll call them "my_base_expansions"
    #    That is effectively the parent's expansions combined with mine (OR pattern).
    my_base_expansions = combined_paths_from_parent

    # 3) Endpoint routes: for each endpoint subpath, produce a single OR pattern that covers
    #    (base expansions) x (endpoint expansion).
    endpoint_routes = @endpoint_defs.map do |(endpoint_subpath, *endpoint_args)|
      # Expand subpath
      ep_expansions = expand_single_subpath(endpoint_subpath)

      # Combine base expansions with endpoint expansions in a cartesian product, then OR them
      final_regex_str = or_pattern_for(cartesian_combine(my_base_expansions, ep_expansions))

      {
        route: Regexp.new("^#{final_regex_str}$"),
        filters: effective_filters_with_endpoint(endpoint_args)
      }
    end

    # 4) A route for this location block itself (without subpaths).
    #    The OR pattern for my_base_expansions (like '^/(?:foo|bar)$' ).
    #    If I have route specs, we produce that route. If you want a route even with no route_specs, adapt.
    location_route = unless @route_specs.empty?
                       pattern_str = or_pattern_for(my_base_expansions) # the expansions themselves
                       {
                         route: Regexp.new("^#{pattern_str}$"),
                         filters: effective_filters
                       }
                     end

    # Final array: child routes first, then endpoints, then my location route
    result = []
    result.concat(child_routes)
    result.concat(endpoint_routes)
    result << location_route if location_route
    result
  end

  ################################
  # Helpers
  ################################

  def validate_path_specs!(specs)
    # 1) If there's more than one raw Ruby Regexp, raise an error
    # 2) If there's 1 raw Ruby Regexp + anything else, also raise an error
    # 3) We can allow *multiple strings*, but if any is a Regexp => can't nest children
    #    We'll actually raise an error if the user tries to create children anyway.
    regexes = specs.select { |s| s.is_a?(Regexp) }
    return unless regexes.size > 1

    raise ArgumentError, 'Cannot have multiple raw Regex route specs in a single location.'
  end

  # Called by flatten_routes to get expansions from the parent's expansions combined with mine
  def combined_paths_from_parent
    if parent
      # get parent's expansions
      pex = parent.combined_paths_from_parent_for_children
      # combine with my route specs
      cartesian_combine(pex, expansions_for(@route_specs))
    else
      # top-level: no parent expansions
      expansions_for(@route_specs)
    end
  end

  # If the parent is a raw Regexp route, that parent wouldn't allow children anyway.
  # But let's define a method the child uses:
  def combined_paths_from_parent_for_children
    # The parent's "my_base_expansions" is the expansions from the parent's route specs,
    # ignoring endpoints. Because a parent's endpoints produce *its own* routes, not expansions for children.
    parent ? parent.combined_paths_from_parent_for_children : [] # recursion upward
  end

  # We override that in a simpler way so it includes the parent's expansions.
  # But to keep it straightforward, we'll store our expansions so children can see them.

  def combined_paths_from_parent_for_children
    # For me, the expansions are expansions_for(@route_specs) combined with parent's expansions
    # if there is one. (Essentially the same logic as combined_paths_from_parent,
    # but we do it for the child's perspective.)
    if parent
      pex = parent.combined_paths_from_parent_for_children
      cartesian_combine(pex, expansions_for(@route_specs))
    else
      expansions_for(@route_specs)
    end
  end

  # Given a single subpath from an endpoint call (a string or a "*"), produce expansions
  def expand_single_subpath(subpath)
    expansions_for([subpath]) # just treat it as a mini specs array
  end

  # Turn an array of specs (strings or at most one Regexp) into an array of expansions (strings).
  # If there's exactly one raw Regexp, we store a single special marker that indicates "raw regexp".
  def expansions_for(specs)
    return [] if specs.empty?

    # If we have a single raw Ruby Regexp, we do not expand it further;
    # but that also means no nesting is allowed. We just store that "as-is".
    if specs.any? { |s| s.is_a? Regexp }
      # Return something that indicates we have a raw Regexp route:
      # We'll store it as an array with one element `[:raw_regex, that_regexp]`
      # so cartesian_combine logic can handle it specially.
      # Let's say we raise if there's more than one
      raise 'Cannot combine a raw Regexp with other strings in the same location.' if specs.size > 1

      [[:raw_regex, specs.first]]
    else
      # We have multiple strings. Convert each to a "fully expanded" sub-regex piece,
      # but do NOT add ^...$ here. We'll do that later.
      # We'll simply store them as strings in "regex-ready" form, i.e. leading slash is included if needed.
      # Actually we only produce the "inner piece" so we can do `^(?: piece1 | piece2 )$`.
      specs.map do |string_spec|
        # remove leading slash
        string_spec = string_spec.sub(%r{^/}, '')
        # if empty => it means "/", so let's keep it blank
        string_spec
      end
    end
  end

  # Combine two arrays of expansions in a cartesian way.
  # If either array has a raw_regexp, we raise if the other is non-empty or also raw.
  # If both are pure string expansions, we produce new expansions for each combination.
  def cartesian_combine(parent_exps, child_exps)
    return child_exps if parent_exps.empty?
    return parent_exps if child_exps.empty?

    # If parent_exps has raw_regexp
    if parent_exps.size == 1 && parent_exps.first.is_a?(Array) && parent_exps.first.first == :raw_regex
      # That means parent's route is a raw Regexp => no children allowed
      # (the problem statement: "if any route is a raw Ruby Regexp, no nesting allowed")
      raise 'Cannot nest under a raw Regexp route.'
    end

    if child_exps.size == 1 && child_exps.first.is_a?(Array) && child_exps.first.first == :raw_regex
      # child is also a raw regex => not allowed in combination
      raise 'Cannot nest a raw Regexp route under a parent string route.'
    end

    # Both are purely strings => cartesian
    results = []
    parent_exps.each do |p|
      child_exps.each do |c|
        # combine with a slash if needed (unless p or c are empty)
        joined = [p, c].reject(&:empty?).join('/')
        results << joined
      end
    end
    results
  end

  # Turn an array of expansions (which are strings like "users/(?<id>[^/]+)" or blank "")
  # into a big OR group. e.g. (?: /users/xxx | /whatever )
  # We'll inject a leading slash if not empty, then create the group.
  def or_pattern_for(expansions)
    return '' if expansions.empty?

    # If expansions has an array that starts with [:raw_regex, /someRegex/], that means no strings
    # but the user is making a direct Regexp. But we already handle that by disallowing nesting.
    # So we shouldn't see it here unless it's from the top location with no parent.
    # In that case let's do a direct pass-through:
    if expansions.size == 1 && expansions.first.is_a?(Array) && expansions.first.first == :raw_regex
      raw = expansions.first.last
      return raw.source # Use the raw Regexp's source
    end

    # For each expansion, we do a slash plus expansion if expansion isn't blank
    # If expansion is blank => it's basically "/"
    pattern_pieces = expansions.map do |exp|
      if exp.empty?
        '' # => means top-level "/"
      else
        # We'll parse param placeholders into named captures.
        # So let's do the same param expansions as in the earlier snippet:
        segment_to_regex_with_slash(exp)
      end
    end

    # Join them with '|'
    joined = pattern_pieces.join('|')

    "(?:#{joined})"
  end

  # Take a string like "users/:id(\d+)" and produce "users/(?<id>\d+)" with the correct param expansions,
  # then put a leading slash. If blank => just slash. We'll do a quick parse (like the earlier snippet).
  def segment_to_regex_with_slash(path_str)
    return '' if path_str == ''

    segments = path_str.split('/')

    converted = segments.map do |seg|
      # wildcard?
      next '.*' if seg == '*'

      # :param(...)?
      if seg =~ /^:([A-Za-z_]\w*)(?:\(([^)]*)\))?$/
        param_name = Regexp.last_match(1)
        custom     = Regexp.last_match(2)
        if custom && !custom.empty?
          "(?<#{param_name}>#{custom})"
        else
          "(?<#{param_name}>[^/]+)"
        end
      else
        Regexp.escape(seg)
      end
    end

    converted.join('/')
  end

  ################################
  # Filter merging
  ################################

  def effective_filters
    # gather from root -> self, overriding duplicates
    merged = merge_ancestor_filters
    # turn into array
    merged.map { |k, v| { type: k, params: v } }
  end

  def effective_filters_with_endpoint(endpoint_args)
    arr = effective_filters
    # endpoint filter last
    ep_filter_params = endpoint_args.dup
    ep_filter_params << @controller_class if @controller_class
    arr << { type: :endpoint, params: ep_filter_params }
    arr
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

################################
# Example usage
################################

if $0 == __FILE__
  dsl = RouterDSL.new do
    location 'http://*' do
      redirect to: 'https://{host}/{path}', status: 301
    end

    location 'http://www.google.com/', 'https://www.foo.bar.com' do
      # Authentication
      basic_auth credential_pairs: [[ENV['BASIC_USER_EMAIL'], ENV['BASIC_USER_PASSWORD']]]
      jwt_auth secret: ENV['JWT_SECRET'], algorithm: 'HS256', verification: 'RS256', public_key: 'public.pem'
      api_key_auth allowed_keys: ENV['API_KEYS'] ? ENV['API_KEYS'].split(',') : []

      # Compression
      compress types: ['text/html', 'text/css', 'application/javascript'],
               threshold: 1024,
               algorithms: %w[gzip deflate]

      # Rate Limiting
      rate_limit :api_rate, limit: '1000 requests/hour', burst: 50

      # CORS
      cors allow_origins: ['https://example.com'], methods: %i[GET POST], apply_to: [:api]

      # Static assets

      location '/users', '/members' do
        file_server dir: 'public', index: 'index.html', default: 'index.html'
        controller 'User'
        endpoint ':id(\d+)', :get_user
      end
    end
  end

  flattened = dsl.flatten_routes
  require 'debug'
  bb
end
