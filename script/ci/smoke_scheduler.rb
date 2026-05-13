# frozen_string_literal: true

require "itsi/scheduler"

started_at = Process.clock_gettime(Process::CLOCK_MONOTONIC)
scheduler = Itsi::Scheduler.new
Fiber.set_scheduler(scheduler)
results = Queue.new

2.times do |idx|
  Fiber.schedule do
    sleep 0.05
    results << idx
  end
end

scheduler.run
elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - started_at
values = 2.times.map { results.pop }

raise "Scheduler smoke test did not resume both fibers" unless values.sort == [0, 1]
raise "Scheduler smoke test took too long: #{elapsed}" unless elapsed < 0.2

puts "scheduler smoke ok"
