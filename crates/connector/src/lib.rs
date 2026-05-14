#[cfg(feature = "event-dispatcher")]
mod dispatcher;
mod error;
#[cfg(feature = "panel-connector")]
mod panel;
#[cfg(feature = "event-dispatcher")]
mod registry;

#[cfg(feature = "event-dispatcher")]
pub use dispatcher::{spawn_event_dispatcher, EventDispatcherHandle, EventDispatcherOptions};
pub use error::{ConnectorError, ConnectorResult};
#[cfg(feature = "panel-connector")]
pub use panel::{spawn_push_connector, PushConnectorHandle};
