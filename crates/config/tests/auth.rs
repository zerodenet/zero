//! Tests for the shared username/password fallback resolver.
//!
//! Migrated from the inline `#[cfg(test)] mod tests` in
//! `crates/config/src/auth.rs`.

use zero_config::auth::resolve_username_password;

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
