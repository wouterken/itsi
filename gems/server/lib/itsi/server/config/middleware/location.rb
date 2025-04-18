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
            routes: Array(Type(String)),
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
          :extensions, :content_types, :accepts, :block

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
          @methods = params[:methods]
          @protocols = params[:protocols] | params[:schemes]
          @hosts = params[:hosts]
          @ports = params[:ports]
          @extensions = params[:extensions]
          @content_types = params[:content_types]
          @accepts = params[:accepts]
          @block = block
        end

        def http_methods
          @methods
        end

        def build!
          build_child = lambda {
            location.children << DSL.new(
              location,
              routes: routes,
              methods: Array(http_methods) | location.http_methods,
              protocols: Array(protocols) | location.protocols,
              hosts: Array(hosts) | location.hosts,
              ports: Array(ports) | location.ports,
              extensions: Array(extensions) | location.extensions,
              content_types: Array(content_types) | location.content_types,
              accepts: Array(accepts) | location.accepts,
              controller: location.controller,
              &block
            )
          }
          if location.parent.nil?
            location.options[:middleware_loaders] << build_child
          else
            build_child[]
          end
        end

      end
    end
  end
end
