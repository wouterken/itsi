module Itsi
  # This trap is necessary for debuggers and similar which intercept certain signals
  # then attempt to restore these to the previous signal when finished.
  # If the previous signal handler was registered in native code, this restoration doesn't
  # work as expected and the native signal handler is lost.
  # We intercept restored signals here and reinstate the Itsi server signal handlers
  # (if the server is still running).
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
