[Kernel, Signal].each do |receiver|
  receiver.singleton_class.prepend(
    Module.new do
      define_method(:trap) do |signal, command = nil, &block|
        if ["DEFAULT", ""].include?(command.to_s) && block.nil?
          Itsi::Server.reset_signal_handlers
        else
          super(signal, command, &block)
        end
      end
    end
  )
end

[Object].each do |receiver|
  receiver.include(
    Module.new do
      define_method(:trap) do |signal, command = nil, &block|
        if ["DEFAULT", ""].include?(command.to_s) && block.nil?
          Itsi::Server.reset_signal_handlers
        else
          super(signal, command, &block)
        end
      end
    end
  )
end
