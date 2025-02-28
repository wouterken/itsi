use magnus::{function, prelude::*, Error, Ruby};

fn hello(subject: String) -> String {
    format!("Hello from Rust, {subject}!")
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("Itsi")?;
    let class = module.define_class("Server", ruby.class_object())?;
    class.define_method("hello", function!(hello, 1))?;
    Ok(())
}
