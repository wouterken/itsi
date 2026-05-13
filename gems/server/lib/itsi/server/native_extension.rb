# frozen_string_literal: true

require "rbconfig"

module Itsi
  class Server
    module NativeExtension
      module_function

      def require!
        ruby_abi = RUBY_VERSION[/\A\d+\.\d+/]

        if ruby_abi && versioned_binary_present?(ruby_abi)
          begin
            require_relative "#{ruby_abi}/itsi_server"
            return
          rescue LoadError
            # Fall back to the source-built extension when a packaged binary
            # exists but cannot be loaded on this machine.
          end
        end

        require_relative "itsi_server"
      end

      def versioned_binary_present?(ruby_abi)
        binary_path = File.join(__dir__, ruby_abi, "itsi_server.#{RbConfig::CONFIG.fetch("DLEXT")}")
        File.exist?(binary_path)
      end
    end
  end
end

Itsi::Server::NativeExtension.require!
