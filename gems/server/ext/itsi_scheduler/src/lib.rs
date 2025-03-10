use itsi_scheduler::ItsiScheduler;
use magnus::{function, method, Error, Module, Object, Ruby};
mod itsi_scheduler;

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    itsi_tracing::init();
    let module = ruby.define_module("Itsi")?;
    let scheduler = module.define_class("Scheduler", ruby.class_object())?;
    scheduler.define_singleton_method("new", function!(ItsiScheduler::new, 0))?;
    scheduler.define_method("io_wait", method!(ItsiScheduler::io_wait, 3))?;
    scheduler.define_method("kernel_sleep", method!(ItsiScheduler::kernel_sleep, 1))?;
    scheduler.define_method("process_wait", method!(ItsiScheduler::process_wait, 2))?;
    scheduler.define_method(
        "address_resolve",
        method!(ItsiScheduler::address_resolve, 1),
    )?;
    scheduler.define_method("block", method!(ItsiScheduler::block, -1))?;
    scheduler.define_method("unblock", method!(ItsiScheduler::unblock, 2))?;
    scheduler.define_method("scheduler_close", method!(ItsiScheduler::run, 0))?;
    scheduler.define_method("run", method!(ItsiScheduler::run, 0))?;
    scheduler.define_method("shutdown", method!(ItsiScheduler::shutdown, 0))?;
    scheduler.define_method("yield", method!(ItsiScheduler::scheduler_yield, 0))?;
    scheduler.define_method("fiber", method!(ItsiScheduler::fiber, -1))?;
    Ok(())
}
