//! Per-protocol authentication contract and the shared username/password
//! fallback resolver.
//!
//! Each protocol variant declares its [`AuthRequirement`] (see
//! [`crate::model::InboundProtocolConfig::auth_requirement`] and
//! [`crate::model::OutboundProtocolConfig::auth_requirement`]). Protocols that
//! require [`AuthRequirement::UsernamePassword`] share a single fallback policy
//! implemented in [`resolve_username_password`]: if the configured username is
//! omitted, the password is used in its place; if both are omitted, no
//! authentication is intended.
//!
//! This is the single owner of that policy — protocol implementations and the
//! crypto layer stay pure and receive already-resolved credentials.

/// The authentication contract a protocol expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthRequirement {
    /// No authentication, ever (e.g. `direct`, `block`, `http_connect` inbound).
    None,
    /// Password only; no username concept (e.g. `shadowsocks`, `trojan`, `hysteria2`).
    PasswordOnly,
    /// Username + password. A missing username is filled from the password.
    /// Missing both means the user does not intend to authenticate.
    UsernamePassword,
    /// Authentication is structurally different or optional
    /// (e.g. `vless` UUID, `vmess` id).
    Other,
}

/// Resolve the username for a [`AuthRequirement::UsernamePassword`] protocol.
///
/// Empty strings are treated as absent.
/// - If a username is present, it is kept.
/// - Otherwise, if a password is present, it is used as the username.
/// - If both are absent, the protocol is treated as anonymous (`None`).
pub fn resolve_username_password(
    username: Option<&str>,
    password: Option<&str>,
) -> Option<String> {
    let username = username.filter(|u| !u.is_empty());
    let password = password.filter(|p| !p.is_empty());
    match (username, password) {
        (Some(u), _) => Some(u.to_owned()),
        (None, Some(p)) => Some(p.to_owned()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_explicit_username() {
        assert_eq!(
            resolve_username_password(Some("alice"), Some("secret")),
            Some("alice".to_owned())
        );
    }

    #[test]
    fn fills_username_from_password_when_missing() {
        assert_eq!(
            resolve_username_password(None, Some("secret")),
            Some("secret".to_owned())
        );
        assert_eq!(
            resolve_username_password(Some(""), Some("secret")),
            Some("secret".to_owned())
        );
    }

    #[test]
    fn both_absent_means_no_auth() {
        assert_eq!(resolve_username_password(None, None), None);
        assert_eq!(resolve_username_password(Some(""), Some("")), None);
        assert_eq!(resolve_username_password(None, Some("")), None);
    }

    #[test]
    fn username_without_password_is_kept() {
        assert_eq!(
            resolve_username_password(Some("alice"), None),
            Some("alice".to_owned())
        );
    }
}
