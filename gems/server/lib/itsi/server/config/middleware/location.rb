module Itsi
  class Server
    module Config
      class Location < Middleware

        insert_text <<~SNIPPET
        location "${1:/}" do
          $2
        end
        SNIPPET

        detail "Group middleware by route and/or route options"

        schema do
          {
            routes: Array(Or(Type(String), Type(Regexp))),
            methods: Array(Type(String)),
            protocols: Array(Type(String)),
            schemes: Array(Type(String)),
            hosts: Array(Type(String)),
            ports: Array(Type(String)),
            extensions: Array(Type(String)),
            content_types: Array(Type(String)),
            accepts: Array(Type(String)),
            block: Type(Proc)
          }
        end

        attr_accessor :location, :routes, :block, :protocols, :hosts, :ports,
          :extensions, :content_types, :accepts

        def initialize(location,
          *routes,
          methods: [],
          protocols: [],
          schemes: [],
          hosts: [],
          ports: [],
          extensions: [],
          content_types: [],
          accepts: [],
          &block
        )

          @location = location
          params = self.schema.new({
            routes: routes,
            methods: methods,
            protocols: protocols,
            schemes: schemes,
            hosts: hosts,
            ports: ports,
            extensions: extensions,
            content_types: content_types,
            accepts: accepts,
            block: block
          }).to_h
          @routes = params[:routes].empty? ? ["*"] : params[:routes]
          @methods = params[:methods].map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @protocols = (params[:protocols] | params[:schemes]).map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @hosts = params[:hosts].map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @ports = params[:ports].map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @extensions = params[:extensions].map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @content_types = params[:content_types].map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @accepts = params[:accepts].map { |s| s.is_a?(Regexp) ? s : s.to_s }
          @block = block
        end

        def http_methods
          @methods
        end

        def intersect(a, b)
          return b if a.empty?
          return a if b.empty?
          a & b
        end

        def build!
          build_child = lambda {
            child = DSL.new(
              location,
              routes: routes,
              methods: intersect(http_methods, location.http_methods),
              protocols: intersect(protocols, location.protocols),
              hosts: intersect(hosts, location.hosts),
              ports: intersect(ports, location.ports),
              extensions: intersect(extensions, location.extensions),
              content_types: intersect(content_types, location.content_types),
              accepts: intersect(accepts, location.accepts),
              controller: location.controller,
              &block
            )
            child.options[:nested_locations].each(&:call)
            location.children << child
          }
          location.options[:nested_locations] << build_child
        end

      end
    end
  end
end
