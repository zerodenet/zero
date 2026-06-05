#[cfg(feature = "event_dispatcher")]
mod dispatcher;
mod error;
#[cfg(feature = "panel_connector")]
mod panel;
#[cfg(feature = "event_dispatcher")]
mod registry;

#[cfg(feature = "event_dispatcher")]
pub use dispatcher::{spawn_event_dispatcher, EventDispatcherHandle, EventDispatcherOptions};
pub use error::{ConnectorError, ConnectorResult};
#[cfg(feature = "panel_connector")]
pub use panel::{spawn_push_connector, PushConnectorHandle};
