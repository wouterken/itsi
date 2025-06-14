module Itsi
  class Server
    module SchedulerInterface
      # Simple wrapper to instantiate a scheduler, start it,
      # and immediate have it invoke a scheduler proc
      def start_scheduler_loop(scheduler_class, scheduler_task)
        scheduler = scheduler_class.new
        Fiber.set_scheduler(scheduler)
        [scheduler, Fiber.schedule(&scheduler_task)]
      end

      # When running in scheduler mode,
      # each request is wrapped in a Fiber.
      def schedule(app, request)
        Fiber.schedule do
          app.call(request)
        rescue StandardError => e
          request.server_error(e.message)
        end
      end
    end
  end
end
