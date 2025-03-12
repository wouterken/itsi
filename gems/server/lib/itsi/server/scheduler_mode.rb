if defined?(ActiveSupport::IsolatedExecutionState) && !ENV["ITSI_DISABLE_AS_AUTO_FIBER_ISOLATION_LEVEL"]
  Itsi.log_info \
    "ActiveSupport Isolated Execution state detected. Automatically switching to :fiber mode. "\
    "Use ENV['ITSI_DISABLE_AS_AUTO_FIBER_ISOLATION_LEVEL'] to disable this behavior"
  ActiveSupport::IsolatedExecutionState.isolation_level = :fiber
end
