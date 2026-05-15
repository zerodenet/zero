//! Fetch URL-based rule sets, cache them locally, and update the config
//! paths to point at the cache.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use zero_config::{RouteRuleSetConfig, RuleSetSourceType};

/// For each URL-based rule set, fetch and cache to `path`.  If the cache
/// file exists and is newer than `update_interval_seconds`, skip the
/// fetch.  If the fetch fails and a cache file exists, use the stale
/// cache.
///
/// Relative paths are resolved against `base_dir`.
pub fn pre_fetch_rule_sets(rule_sets: &mut [RouteRuleSetConfig], base_dir: Option<&Path>) {
    for rule_set in rule_sets.iter_mut() {
        if rule_set.source_type != RuleSetSourceType::Url {
            continue;
        }
        let Some(ref url) = rule_set.url else {
            tracing::warn!(tag = %rule_set.tag, "url rule set missing `url` field");
            continue;
        };

        let cache_path = resolve_path(&rule_set.path, base_dir);

        // Check if cache is still fresh.
        if is_cache_fresh(&cache_path, rule_set.update_interval_seconds) {
            tracing::debug!(tag = %rule_set.tag, url = %url, "rule set cache is fresh");
            rule_set.path = cache_path.to_string_lossy().to_string();
            continue;
        }

        match ureq::get(url).call() {
            Ok(response) => {
                let content = response.into_string().unwrap_or_default();
                if let Some(parent) = cache_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Err(e) = fs::write(&cache_path, &content) {
                    tracing::warn!(
                        tag = %rule_set.tag, url = %url,
                        path = %cache_path.display(), error = %e,
                        "failed to write rule set cache"
                    );
                    continue;
                }
                tracing::info!(
                    tag = %rule_set.tag, url = %url,
                    path = %cache_path.display(), bytes = content.len(),
                    "rule set fetched and cached"
                );
                rule_set.path = cache_path.to_string_lossy().to_string();
            }
            Err(e) => {
                if cache_path.exists() {
                    tracing::warn!(
                        tag = %rule_set.tag, url = %url, error = %e,
                        "rule set fetch failed; using stale cache"
                    );
                    rule_set.path = cache_path.to_string_lossy().to_string();
                } else {
                    tracing::error!(
                        tag = %rule_set.tag, url = %url, error = %e,
                        "rule set fetch failed; no cache available"
                    );
                }
            }
        }
    }
}

fn is_cache_fresh(cache_path: &Path, update_interval_seconds: u64) -> bool {
    let Ok(meta) = fs::metadata(cache_path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let age_secs = modified
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now - age_secs < update_interval_seconds
}

fn resolve_path(path: &str, base_dir: Option<&Path>) -> PathBuf {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return candidate.to_path_buf();
    }
    match base_dir {
        Some(base_dir) => base_dir.join(candidate),
        None => candidate.to_path_buf(),
    }
}
