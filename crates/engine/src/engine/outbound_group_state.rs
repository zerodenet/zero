use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
pub struct OutboundGroupStateStore {
    selector: Mutex<HashMap<String, String>>,
    urltest: Mutex<HashMap<String, UrlTestGroupState>>,
}

impl OutboundGroupStateStore {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn initialize_selector(&self, tag: &str, selected: &str) {
        self.selector
            .lock()
            .expect("selector group state lock poisoned")
            .insert(tag.to_owned(), selected.to_owned());
    }

    pub fn update_selector(&self, tag: &str, selected: &str) {
        self.selector
            .lock()
            .expect("selector group state lock poisoned")
            .insert(tag.to_owned(), selected.to_owned());
    }

    pub fn selector_selected_outbound(&self, tag: &str) -> Option<String> {
        self.selector
            .lock()
            .expect("selector group state lock poisoned")
            .get(tag)
            .cloned()
    }

    pub fn initialize_urltest(&self, tag: &str, selected: &str) {
        self.urltest
            .lock()
            .expect("urltest group state lock poisoned")
            .insert(
                tag.to_owned(),
                UrlTestGroupState {
                    selected: selected.to_owned(),
                    latency_ms: None,
                    last_checked_unix_ms: None,
                },
            );
    }

    pub fn update_urltest(&self, tag: &str, selected: &str, latency_ms: Option<u64>) {
        self.urltest
            .lock()
            .expect("urltest group state lock poisoned")
            .insert(
                tag.to_owned(),
                UrlTestGroupState {
                    selected: selected.to_owned(),
                    latency_ms,
                    last_checked_unix_ms: Some(unix_timestamp_ms()),
                },
            );
    }

    pub fn selected_outbound(&self, tag: &str) -> Option<String> {
        self.urltest_selected_outbound(tag)
            .or_else(|| self.selector_selected_outbound(tag))
    }

    pub fn urltest_state(&self, tag: &str) -> Option<UrlTestGroupState> {
        self.urltest
            .lock()
            .expect("urltest group state lock poisoned")
            .get(tag)
            .cloned()
    }

    pub fn urltest_selected_outbound(&self, tag: &str) -> Option<String> {
        self.urltest
            .lock()
            .expect("urltest group state lock poisoned")
            .get(tag)
            .map(|state| state.selected.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlTestGroupState {
    pub selected: String,
    pub latency_ms: Option<u64>,
    pub last_checked_unix_ms: Option<u64>,
}

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis() as u64
}
