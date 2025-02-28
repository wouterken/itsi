use itsi_tracing::info;
use magnus::{function, Error, Module, Object, Ruby};
use server::Server;

mod server;

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    itsi_tracing::init();

    info!("Initializing Itsi::Server");

    let module = ruby.define_module("Itsi")?;
    let class = module.define_class("Server", ruby.class_object())?;
    class.define_singleton_method("new", function!(Server::new, -1))?;

    Ok(())
}
