pub mod bindings;
pub mod cc;
pub mod utils;

mod rb_config;

pub use rb_config::*;

/// The current RbConfig.
pub fn rb_config() -> RbConfig {
    RbConfig::current()
}
