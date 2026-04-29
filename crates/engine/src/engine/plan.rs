use std::collections::HashMap;
use std::time::Duration;

use zero_config::{ClientTlsConfig, OutboundGroupKind, OutboundProtocolConfig, RuntimeConfig};

use super::error::EngineError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TargetId(usize);

impl TargetId {
    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnginePlan {
    targets: Box<[TargetNode]>,
    targets_by_tag: HashMap<String, TargetId>,
    selector_groups: Box<[TargetId]>,
    urltest_groups: Box<[TargetId]>,
}

impl EnginePlan {
    pub fn build(config: &RuntimeConfig) -> Result<Self, EngineError> {
        let targets_by_tag = build_target_index(config)?;
        let mut targets = Vec::with_capacity(targets_by_tag.len());
        let mut selector_groups = Vec::new();
        let mut urltest_groups = Vec::new();

        for outbound in &config.outbounds {
            let kind = match &outbound.protocol {
                OutboundProtocolConfig::Direct => TargetKind::Outbound(OutboundTarget::Direct),
                OutboundProtocolConfig::Block => TargetKind::Outbound(OutboundTarget::Block),
                OutboundProtocolConfig::Socks5 {
                    server,
                    port,
                    username,
                    password,
                } => TargetKind::Outbound(OutboundTarget::Socks5 {
                    server: server.clone(),
                    port: *port,
                    username: username.clone(),
                    password: password.clone(),
                }),
                OutboundProtocolConfig::Vless {
                    server,
                    port,
                    id,
                    tls,
                    ws,
                } => TargetKind::Outbound(OutboundTarget::Vless {
                    server: server.clone(),
                    port: *port,
                    id: id.clone(),
                    tls: tls.clone(),
                    ws: ws.clone(),
                }),
            };

            targets.push(TargetNode {
                tag: outbound.tag.clone(),
                kind,
            });
        }

        for group in &config.outbound_groups {
            let group_id = target_id_by_tag(&targets_by_tag, group.tag())?;
            let kind = match &group.group {
                OutboundGroupKind::Selector { outbounds, .. } => {
                    selector_groups.push(group_id);
                    TargetKind::Selector(SelectorGroupPlan {
                        members: resolve_members(outbounds, &targets_by_tag)?,
                        initial_member: target_id_by_tag(
                            &targets_by_tag,
                            group
                                .active_outbound()
                                .ok_or_else(|| EngineError::InvalidPlan {
                                    message: format!(
                                        "selector group `{}` does not have an initial member",
                                        group.tag()
                                    ),
                                })?,
                        )?,
                    })
                }
                OutboundGroupKind::Fallback { outbounds } => {
                    TargetKind::Fallback(FallbackGroupPlan {
                        members: resolve_members(outbounds, &targets_by_tag)?,
                    })
                }
                OutboundGroupKind::UrlTest {
                    outbounds,
                    url,
                    interval_seconds,
                } => {
                    urltest_groups.push(group_id);
                    TargetKind::UrlTest(UrlTestGroupPlan {
                        members: resolve_members(outbounds, &targets_by_tag)?,
                        initial_member: target_id_by_tag(
                            &targets_by_tag,
                            group
                                .active_outbound()
                                .ok_or_else(|| EngineError::InvalidPlan {
                                    message: format!(
                                        "urltest group `{}` does not have an initial member",
                                        group.tag()
                                    ),
                                })?,
                        )?,
                        url: url.clone(),
                        interval: Duration::from_secs(*interval_seconds),
                    })
                }
            };

            targets.push(TargetNode {
                tag: group.tag.clone(),
                kind,
            });
        }

        Ok(Self {
            targets: targets.into_boxed_slice(),
            targets_by_tag,
            selector_groups: selector_groups.into_boxed_slice(),
            urltest_groups: urltest_groups.into_boxed_slice(),
        })
    }

    pub fn target_id(&self, tag: &str) -> Option<TargetId> {
        self.targets_by_tag.get(tag).copied()
    }

    pub fn target(&self, id: TargetId) -> Option<&TargetNode> {
        self.targets.get(id.index())
    }

    pub fn selector_groups(&self) -> &[TargetId] {
        &self.selector_groups
    }

    pub fn urltest_groups(&self) -> &[TargetId] {
        &self.urltest_groups
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetNode {
    tag: String,
    kind: TargetKind,
}

impl TargetNode {
    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub fn kind(&self) -> &TargetKind {
        &self.kind
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetKind {
    Outbound(OutboundTarget),
    Selector(SelectorGroupPlan),
    Fallback(FallbackGroupPlan),
    UrlTest(UrlTestGroupPlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboundTarget {
    Direct,
    Block,
    Socks5 {
        server: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },
    Vless {
        server: String,
        port: u16,
        id: String,
        tls: Option<ClientTlsConfig>,
        ws: Option<zero_config::WebSocketConfig>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorGroupPlan {
    members: Box<[TargetId]>,
    initial_member: TargetId,
}

impl SelectorGroupPlan {
    pub fn members(&self) -> &[TargetId] {
        &self.members
    }

    pub fn initial_member(&self) -> TargetId {
        self.initial_member
    }

    pub fn contains_member(&self, target: TargetId) -> bool {
        self.members.contains(&target)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackGroupPlan {
    members: Box<[TargetId]>,
}

impl FallbackGroupPlan {
    pub fn members(&self) -> &[TargetId] {
        &self.members
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlTestGroupPlan {
    members: Box<[TargetId]>,
    initial_member: TargetId,
    url: String,
    interval: Duration,
}

impl UrlTestGroupPlan {
    pub fn members(&self) -> &[TargetId] {
        &self.members
    }

    pub fn initial_member(&self) -> TargetId {
        self.initial_member
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }
}

fn build_target_index(config: &RuntimeConfig) -> Result<HashMap<String, TargetId>, EngineError> {
    let mut targets_by_tag =
        HashMap::with_capacity(config.outbounds.len() + config.outbound_groups.len());

    for (index, outbound) in config.outbounds.iter().enumerate() {
        insert_target_tag(&mut targets_by_tag, &outbound.tag, TargetId(index))?;
    }

    let group_base = config.outbounds.len();
    for (offset, group) in config.outbound_groups.iter().enumerate() {
        insert_target_tag(
            &mut targets_by_tag,
            group.tag(),
            TargetId(group_base + offset),
        )?;
    }

    Ok(targets_by_tag)
}

fn insert_target_tag(
    targets_by_tag: &mut HashMap<String, TargetId>,
    tag: &str,
    id: TargetId,
) -> Result<(), EngineError> {
    match targets_by_tag.insert(tag.to_owned(), id) {
        Some(_) => Err(EngineError::InvalidPlan {
            message: format!("duplicate target tag `{tag}` found while building engine plan"),
        }),
        None => Ok(()),
    }
}

fn resolve_members(
    members: &[String],
    targets_by_tag: &HashMap<String, TargetId>,
) -> Result<Box<[TargetId]>, EngineError> {
    let mut resolved = Vec::with_capacity(members.len());
    for member in members {
        resolved.push(target_id_by_tag(targets_by_tag, member)?);
    }

    Ok(resolved.into_boxed_slice())
}

fn target_id_by_tag(
    targets_by_tag: &HashMap<String, TargetId>,
    tag: &str,
) -> Result<TargetId, EngineError> {
    targets_by_tag
        .get(tag)
        .copied()
        .ok_or_else(|| EngineError::InvalidPlan {
            message: format!("target tag `{tag}` is missing from engine plan"),
        })
}
