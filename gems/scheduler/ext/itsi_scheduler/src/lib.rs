use itsi_tracing::info;
use magnus::{function, prelude::*, Error, Ruby};

fn hello(subject: String) -> String {
    format!("Hello from Rust, {subject}!")
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    itsi_tracing::init();
    info!("Initializing Itsi::Scheduler");

    let module = ruby.define_module("Itsi")?;
    let class = module.define_class("Scheduler", ruby.class_object())?;
    class.define_method("hello", function!(hello, 1))?;
    Ok(())
}
