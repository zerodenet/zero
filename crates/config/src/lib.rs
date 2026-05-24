#![allow(clippy::large_enum_variant)]

mod compile;
mod error;
mod model;
mod rule_sets;
mod validate;

pub use error::ConfigError;
pub use model::{
    ApiConfig, ClientTlsConfig, ControlApiConfig, DnsCacheConfig, DnsConfig, DnsRouteConfig,
    DnsServerConfig, EventSinkConfig, FakeIpConfig, FallbackConfig, GrpcConfig, H2Config,
    HookConfig, HttpUpgradeConfig, InboundConfig, InboundProtocolConfig, InboundRealityConfig,
    ListenConfig, LoadBalanceStrategy, LogConfig, LogFileConfig, LogRateLimit, ModeConfig,
    OutboundConfig, OutboundGroupConfig, OutboundGroupKind, OutboundProtocolConfig, PushConfig,
    QuicConfig, RealityConfig, RouteActionConfig, RouteConfig, RouteRuleConfig, RouteRuleSetConfig,
    RuleConditionConfig, RuleSetFormatConfig, RuleSetSourceType, RuntimeConfig,
    RuntimeOptionsConfig, Socks5UserConfig, SplitHttpConfig, TlsConfig, UrlRewriteRule,
    VlessUserConfig, VmessUserConfig, WebSocketConfig,
};
