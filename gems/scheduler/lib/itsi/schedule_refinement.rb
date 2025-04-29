module Itsi
  module ScheduleRefinement
    # Useful helper functions for using cooperative multi-tasking in Ruby.
    # Opt-in to usage by executing `using Itsi::ScheduleRefinement` in any places
    # you intend to use it.
    #
    # After this you can do things like the following
    #
    # 1. Launch batch concurrent fire-and-forget jobs.
    # * 100.times.schedule_each{ sleep 0.1 }
    #
    # 2. Launch batch concurrent transofmrs
    # See how `schedule_map` retains ordering, despite sleeping for randomized amount of time.
    #
    # * 100.times.schedule_map{|i| sleep Random.rand(0.0..0.05); i }
    #
    # 3. Manually organize fibers to run concurrently.
    #
    # require "net/http"
    # schedule do
    #   req1, req2 = Queue.new, Queue.new
    #   schedule do
    #     puts "Making request 1"
    #     req1 << Net::HTTP.get(URI("http://httpbin.org/get"))
    #     puts "Finished request 1"
    #   end
    #
    #   schedule do
    #     puts "Making request 2"
    #     req2 << Net::HTTP.get(URI("http://httpbin.org/get"))
    #     puts "Finished request 2"
    #   end
    #
    #   res1, res2 = [req1, req2].map(&:pop)
    # end
    refine Kernel do
      private def schedule(&blk) # rubocop:disable Metrics/MethodLength
        return unless blk

        if Fiber.scheduler.nil?
          result = nil
          Thread.new do
            Fiber.set_scheduler Itsi::Scheduler.new
            Fiber.schedule { result = blk.call }
          end.join
          result
        else
          Fiber.schedule(&blk)
        end
      end
    end

    module EnumerableExtensions
      using ScheduleRefinement
      def schedule_each(&block)
        enum = Enumerator.new do |y|
          schedule do
            each { |item| schedule{ y.yield(item) } }
          end
        end

        block_given? ? enum.each(&block) : enum.each
      end

      def schedule_map(&block)
        return Enumerator.new do |y|
          schedule do
            with_index.each_with_object([]) do |(item, index), agg|
              schedule do
                agg[index] = (y << item)
              end
            end
          end
        end.map unless block_given?
        schedule do
          with_index.each_with_object([]) do |(item, index), agg|
            schedule do
              agg[index] = block[item]
            end
          end
        end
      end
    end


    refine Enumerator do
      define_method(:schedule_each, EnumerableExtensions.instance_method(:schedule_each))
      define_method(:schedule_map, EnumerableExtensions.instance_method(:schedule_map))
    end

    refine Enumerable do
      define_method(:schedule_each, EnumerableExtensions.instance_method(:schedule_each))
      define_method(:schedule_map, EnumerableExtensions.instance_method(:schedule_map))
    end
  end
end
