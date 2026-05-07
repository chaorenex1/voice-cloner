use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
use serde::Serialize;

static TRACE_COUNTER: AtomicU64 = AtomicU64::new(1);
static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TraceId(String);

impl TraceId {
    pub fn new(scope: &str) -> Self {
        let sequence = TRACE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let timestamp = Utc::now().format("%Y%m%d%H%M%S%3f");
        Self(format!("{scope}-{timestamp}-{sequence}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

pub fn new_entity_id(prefix: &str) -> String {
    let sequence = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = Utc::now().format("%Y%m%d%H%M%S%3f");
    format!("{prefix}-{timestamp}-{sequence}")
}

#[cfg(test)]
mod tests {
    use super::{new_entity_id, TraceId};

    #[test]
    fn trace_ids_include_scope_and_are_unique() {
        let first = TraceId::new("settings");
        let second = TraceId::new("settings");

        assert!(first.as_str().starts_with("settings-"));
        assert_ne!(first, second);
    }

    #[test]
    fn entity_ids_include_prefix_and_are_unique() {
        let first = new_entity_id("session");
        let second = new_entity_id("session");

        assert!(first.starts_with("session-"));
        assert_ne!(first, second);
    }
}
