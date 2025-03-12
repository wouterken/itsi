# frozen_string_literal: true

require "minitest/reporters"
Minitest::Reporters.use! Minitest::Reporters::SpecReporter.new

require "itsi/scheduler"
require 'debug'
module Itsi::Scheduler::TestHelper
  SchedulerClass = Itsi::Scheduler

  def with_scheduler(join: true, report_on_exception: false)
    Thread.new do
      Thread.current.report_on_exception = report_on_exception
      scheduler = SchedulerClass.new
      Fiber.set_scheduler(scheduler)
      Fiber.schedule do
        yield scheduler
      end
    end.yield_self do |thread|
      thread.join if join
      thread
    end
  end
end
