module Itsi
  module Signals
    DEFAULT_SIGNALS = ["DEFAULT", ""].freeze
    module SignalTrap
      def self.trap(signal, *args, &block)
        if DEFAULT_SIGNALS.include?(command.to_s) && block.nil?
          Itsi::Server.reset_signal_handlers
          nil
        else
          super(signal, *args, &block)
        end
      end
    end
  end
end

[Kernel, Signal].each do |receiver|
  receiver.singleton_class.prepend(Itsi::Signals::SignalTrap)
end

[Object].each do |receiver|
  receiver.include(Itsi::Signals::SignalTrap)
end
