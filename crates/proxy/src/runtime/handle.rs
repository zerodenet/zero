//! Control-plane handle facade.
//!
//! Keep the root thin so command/query/event interception details do not
//! accumulate back into a single runtime file.

mod command;
mod event;
mod model;
mod query;
mod util;

pub use model::ProxyHandle;
