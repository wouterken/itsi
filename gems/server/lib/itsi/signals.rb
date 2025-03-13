module Itsi
  module Signals
    DEFAULT_SIGNALS = ["DEFAULT", ""].freeze
    module TrapInterceptor
      def trap(signal, command = nil, &block)
        return super unless DEFAULT_SIGNALS.include?(command.to_s) && block.nil?
        Itsi::Server.reset_signal_handlers
      end
    end
    [Kernel, Signal].each do |receiver|
      receiver.singleton_class.prepend(TrapInterceptor)
    end

    [Object].each do |receiver|
      receiver.include(TrapInterceptor)
    end
  end
end
