# frozen_string_literal: true

$LOAD_PATH.unshift File.expand_path("../lib", __dir__)
require "itsi/scheduler"

require "minitest/autorun"
require 'async'
module Itsi::Scheduler::TestHelper
  SchedulerClass = Itsi::Scheduler

  def with_scheduler(join: true)
    Thread.new do
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
