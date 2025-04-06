# frozen_string_literal: true

require "ruby_lsp/addon"
puts "Here we go"
module RubyLsp
  module Itsi
    class Addon < ::RubyLsp::Addon

      def name
        "Ruby LSP Itsi"
      end

      def version
        "0.1.0"
      end

      def create_completion_listener(response_builder, node_context, dispatcher, uri)
        return unless uri.to_s.end_with?("Itsi.rb")
        @in_itsi_file = true
        CompletionListener.new(response_builder, node_context, dispatcher, uri)
      end

      def create_hover_listener(response_builder, node_context, dispatcher)
        hl =  dispatcher.listeners[:on_call_node_enter].find{|c| c.kind_of?(RubyLsp::Listeners::Hover)}

        return unless hl.instance_variable_get("@path").to_s.end_with?("Itsi.rb")
        HoverListener.new(response_builder, node_context, dispatcher)
      end
    end

    class HoverListener
      def initialize(response_builder, node_context, dispatcher)
        @response_builder = response_builder
        @node_context = node_context
        @dispatcher = dispatcher

        # Register for call nodes (you could also use other events if more appropriate)
        dispatcher.register(self, :on_call_node_enter)
      end

      def on_call_node_enter(node)
        # Check if this is the call for foo_bar; you might add additional logic if needed.
        case node.message
        when "log_level"
          # Push a simple hover response.
          @response_builder.push(
            "Method **log_level**: A method that takes a single parameter `a`.",
            category: :documentation
          )
        when "shutdown_timeout"
          # Push a simple hover response.
          @response_builder.push(
            "Method **shutdown_timeout**: Number of seconds to wait for a graceful shutdown before forcefully shutting down.",
            category: :documentation
          )
        when "log"
          # Push a simple hover response.
          @response_builder.push(
            "Method **log**: A method that takes a single parameter `a`.",
            category: :documentation
          )
        end
      end
    end

    class CompletionListener
      def initialize(response_builder, node_context, dispatcher, uri)
        @response_builder = response_builder
        @node_context = node_context
        @uri = uri
        @dispatcher = dispatcher

        # Register for call nodes to handle method completions
        dispatcher.register(self, :on_call_node_enter)
      end

      def on_call_node_enter(node)
        # Only handle method calls that are being typed (no arguments yet)
        return unless node.arguments.nil?
        puts "Adding log level completion"
        # Add our completion item
        @response_builder << Interface::CompletionItem.new(
          label: "log_level",
          kind: Constant::CompletionItemKind::METHOD,
          detail: "log_level a ",
          documentation: "A method that takes a single parameter 'a'",
          insert_text: "log_level ${1|:info,:debug,:warn,:error|}",
          insert_text_format: Constant::InsertTextFormat::SNIPPET,
        )

        @response_builder << Interface::CompletionItem.new(
          label: "bind",
          kind: Constant::CompletionItemKind::METHOD,
          detail: "Bind to an interface",
          documentation: "A method that takes a single parameter 'a'",
          insert_text: "bind ${1|'https://0.0.0.0','http://0.0.0.0','http://0.0.0.0:3000','https://0.0.0.0:3000','http://127.0.0.1','https://127.0.0.1'|}",
          insert_text_format: Constant::InsertTextFormat::SNIPPET,
        )

        @response_builder << Interface::CompletionItem.new(
          label: "log_format",
          kind: Constant::CompletionItemKind::METHOD,
          detail: "log_format",
          documentation: "A method that takes a single parameter 'a'",
          insert_text: "log_format ${1|:plain,:json,:auto|}",
          insert_text_format: Constant::InsertTextFormat::SNIPPET,
        )
      end
    end
  end
end
