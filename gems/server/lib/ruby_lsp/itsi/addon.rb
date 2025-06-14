# frozen_string_literal: true

require "ruby_lsp/addon"
require "itsi/server"

module RubyLsp
  module Itsi
    class Addon < ::RubyLsp::Addon # rubocop:disable Style/Documentation
      def activate(_global_state, message_queue)
        @message_queue = message_queue
      end

      def deactivate; end

      def name
        "Ruby LSP Itsi"
      end

      def version
        "0.1.0"
      end

      def create_completion_listener(response_builder, node_context, dispatcher, uri)
        return unless uri.to_s =~ /itsi.rb$/i

        @in_itsi_file = true
        CompletionListener.new(response_builder, node_context, dispatcher, uri)
      end

      def create_hover_listener(response_builder, node_context, dispatcher)
        hl = dispatcher.listeners[:on_call_node_enter].find { |c| c.is_a?(RubyLsp::Listeners::Hover) }
        return unless hl.instance_variable_get("@path").to_s =~ /itsi.rb$/i

        HoverListener.new(response_builder, node_context, dispatcher)
      end
    end

    class HoverListener # rubocop:disable Style/Documentation
      def initialize(response_builder, node_context, dispatcher)
        @response_builder = response_builder
        @node_context = node_context
        @dispatcher = dispatcher

        @options_by_name = ::Itsi::Server::Config::Option.subclasses.group_by(&:option_name).transform_values(&:first)
        @middlewares_by_name = ::Itsi::Server::Config::Middleware.subclasses.group_by(&:middleware_name).transform_values(&:first)
        # Register for call nodes for hover events
        dispatcher.register(self, :on_call_node_enter)
      end

      def on_call_node_enter(node) # rubocop:disable Metrics/MethodLength
        if (matched_class = @options_by_name[node.message.to_sym])
          @response_builder.push(
            matched_class.documentation,
            category: :documentation
          )
        elsif (matched_class = @middlewares_by_name[node.message.to_sym])
          @response_builder.push(
            matched_class.documentation,
            category: :documentation
          )
        end
      end
    end

    class CompletionListener # rubocop:disable Style/Documentation
      def initialize(response_builder, node_context, dispatcher, uri)
        @response_builder = response_builder
        @node_context = node_context
        @uri = uri
        @dispatcher = dispatcher

        # Register for method call completions
        dispatcher.register(self, :on_call_node_enter)
        # Also register for completion item resolution requests
        dispatcher.register(self, :completion_item_resolve)
      end

      def on_call_node_enter(node) # rubocop:disable Metrics/AbcSize
        # Only handle method calls that are being typed (i.e. no arguments yet)
        return unless node.arguments.nil?

        ::Itsi::Server::Config::Option.subclasses.each do |option|
          completion_item = Interface::CompletionItem.new(
            label: option.option_name,
            kind: Constant::CompletionItemKind::METHOD,
            label_details: Interface::CompletionItemLabelDetails.new(
              detail: option.detail,
              description: option.documentation
            ),
            documentation: Interface::MarkupContent.new(
              kind: Constant::MarkupKind::MARKDOWN,
              value: option.documentation
            ),
            insert_text: option.insert_text,
            insert_text_format: Constant::InsertTextFormat::SNIPPET,
            data: {
              delegateCompletion: true
            }
          )
          @response_builder << completion_item
        end

        ::Itsi::Server::Config::Middleware.subclasses.each do |middleware|
          Array(middleware.insert_text).zip(Array(middleware.detail)).each do |insert_text, detail|
            completion_item = Interface::CompletionItem.new(
              label: middleware.middleware_name,
              kind: Constant::CompletionItemKind::METHOD,
              label_details: Interface::CompletionItemLabelDetails.new(
                detail: detail,
                description: middleware.documentation
              ),
              documentation: Interface::MarkupContent.new(
                kind: Constant::MarkupKind::MARKDOWN,
                value: middleware.documentation
              ),
              insert_text: insert_text,
              insert_text_format: Constant::InsertTextFormat::SNIPPET,
              data: {
                delegateCompletion: true
              }
            )
            @response_builder << completion_item
          end
        end
      end
    end
  end
end
