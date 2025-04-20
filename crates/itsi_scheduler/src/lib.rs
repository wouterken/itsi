use itsi_scheduler::ItsiScheduler;
use magnus::{function, method, Class, Error, Module, Object, Ruby};
mod itsi_scheduler;

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    itsi_tracing::init();
    let module = ruby.define_module("Itsi")?;
    let scheduler = module.define_class("Scheduler", ruby.class_object())?;
    scheduler.define_singleton_method("info", function!(ItsiScheduler::class_info, 1))?;
    scheduler.define_alloc_func::<ItsiScheduler>();
    scheduler.define_method("initialize", method!(ItsiScheduler::initialize, 0))?;
    scheduler.define_method("wake", method!(ItsiScheduler::wake, 0))?;
    scheduler.define_method(
        "register_io_wait",
        method!(ItsiScheduler::register_io_wait, 4),
    )?;
    scheduler.define_method("info", method!(ItsiScheduler::info, 1))?;
    scheduler.define_method("debug", method!(ItsiScheduler::debug, 1))?;
    scheduler.define_method("warn", method!(ItsiScheduler::warn, 1))?;
    scheduler.define_method("start_timer", method!(ItsiScheduler::start_timer, 2))?;
    scheduler.define_method("clear_timer", method!(ItsiScheduler::clear_timer, 1))?;
    scheduler.define_method(
        "address_resolve",
        method!(ItsiScheduler::address_resolve, 1),
    )?;
    scheduler.define_method("has_pending_io?", method!(ItsiScheduler::has_pending_io, 0))?;

    scheduler.define_method(
        "fetch_due_timers",
        method!(ItsiScheduler::fetch_due_timers, 0),
    )?;
    scheduler.define_method(
        "fetch_due_events",
        method!(ItsiScheduler::fetch_due_events, 0),
    )?;

    Ok(())
}
