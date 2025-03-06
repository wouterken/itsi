use magnus::{Error, Module, Ruby};

#[magnus::wrap(class = "Itsi::Scheduler", free_immediately, size)]
struct Scheduler {}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    itsi_tracing::init();
    let module = ruby.define_module("Itsi")?;
    let _scheduler = module.define_class("Scheduler", ruby.class_object())?;

    Ok(())
}
