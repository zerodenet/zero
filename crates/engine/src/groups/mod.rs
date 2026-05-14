use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use super::plan::TargetId;

#[derive(Debug, Default)]
pub(crate) struct OutboundGroupStateStore {
    selector: Mutex<HashMap<TargetId, TargetId>>,
    urltest: Mutex<HashMap<TargetId, UrlTestGroupState>>,
}

impl OutboundGroupStateStore {
    pub(crate) fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(crate) fn initialize_selector(&self, group_id: TargetId, selected: TargetId) {
        self.selector
            .lock()
            .expect("selector group state lock poisoned")
            .insert(group_id, selected);
    }

    pub(crate) fn update_selector(&self, group_id: TargetId, selected: TargetId) {
        self.selector
            .lock()
            .expect("selector group state lock poisoned")
            .insert(group_id, selected);
    }

    pub(crate) fn selector_selected_target(&self, group_id: TargetId) -> Option<TargetId> {
        self.selector
            .lock()
            .expect("selector group state lock poisoned")
            .get(&group_id)
            .copied()
    }

    pub(crate) fn initialize_urltest(
        &self,
        group_id: TargetId,
        selected: TargetId,
        members: &[TargetId],
    ) {
        self.urltest
            .lock()
            .expect("urltest group state lock poisoned")
            .insert(
                group_id,
                UrlTestGroupState {
                    selected,
                    latency_ms: None,
                    last_checked_unix_ms: None,
                    members: members
                        .iter()
                        .map(|member_id| UrlTestMemberState {
                            member_id: *member_id,
                            healthy: false,
                            latency_ms: None,
                            last_checked_unix_ms: None,
                            last_error: None,
                            effective_chains: Vec::new(),
                        })
                        .collect(),
                },
            );
    }

    pub(crate) fn update_urltest(
        &self,
        group_id: TargetId,
        selected: TargetId,
        latency_ms: Option<u64>,
        members: Vec<UrlTestMemberState>,
    ) {
        self.urltest
            .lock()
            .expect("urltest group state lock poisoned")
            .insert(
                group_id,
                UrlTestGroupState {
                    selected,
                    latency_ms,
                    last_checked_unix_ms: Some(unix_timestamp_ms()),
                    members,
                },
            );
    }

    pub(crate) fn selected_target(&self, group_id: TargetId) -> Option<TargetId> {
        self.urltest_selected_target(group_id)
            .or_else(|| self.selector_selected_target(group_id))
    }

    pub(crate) fn urltest_state(&self, group_id: TargetId) -> Option<UrlTestGroupState> {
        self.urltest
            .lock()
            .expect("urltest group state lock poisoned")
            .get(&group_id)
            .cloned()
    }

    pub(crate) fn urltest_selected_target(&self, group_id: TargetId) -> Option<TargetId> {
        self.urltest
            .lock()
            .expect("urltest group state lock poisoned")
            .get(&group_id)
            .map(|state| state.selected)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlTestGroupState {
    pub selected: TargetId,
    pub latency_ms: Option<u64>,
    pub last_checked_unix_ms: Option<u64>,
    pub members: Vec<UrlTestMemberState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlTestMemberState {
    pub member_id: TargetId,
    pub healthy: bool,
    pub latency_ms: Option<u64>,
    pub last_checked_unix_ms: Option<u64>,
    pub last_error: Option<String>,
    pub effective_chains: Vec<Vec<TargetId>>,
}

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis() as u64
}
