# Running with a Fiber scheduler enabled but with an ActiveSupport isolation_level set to Thread
# can be dangerous. A thread isolation level means that all fibers sharing a thread can content
# for the same resources, which can lead to race conditions.
# This hook should *only* be disabled if you know there are no such shared resources.
if defined?(ActiveSupport::IsolatedExecutionState) && !ENV["ITSI_DISABLE_AS_AUTO_FIBER_ISOLATION_LEVEL"]
  Itsi.log_info \
    "ActiveSupport Isolated Execution state detected. Automatically switching to :fiber mode. "\
    "Set ITSI_DISABLE_AS_AUTO_FIBER_ISOLATION_LEVEL to disable this behavior"
  ActiveSupport::IsolatedExecutionState.isolation_level = :fiber
end
