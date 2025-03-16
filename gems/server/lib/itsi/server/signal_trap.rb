module Itsi
  module SignalTrap

    DEFAULT_SIGNALS = ["DEFAULT", "", nil].freeze
    INTERCEPTED_SIGNALS = ["INT"].freeze

    def trap(signal, *args, &block)
      unless INTERCEPTED_SIGNALS.include?(signal.to_s) && block.nil? && Itsi::Server.running?
        return super(signal, *args, &block)
      end

      Itsi::Server.reset_signal_handlers
      nil
    end
  end
end

[Kernel, Signal].each do |receiver|
  receiver.singleton_class.prepend(Itsi::SignalTrap)
end

[Object].each do |receiver|
  receiver.include(Itsi::SignalTrap)
end
