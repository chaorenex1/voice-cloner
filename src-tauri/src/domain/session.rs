use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::runtime_params::RuntimeParams;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RealtimeSessionStatus {
    Idle,
    Connecting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeSession {
    pub session_id: String,
    pub trace_id: String,
    pub voice_name: String,
    pub runtime_params: RuntimeParams,
    pub backend_name: String,
    pub status: RealtimeSessionStatus,
    pub websocket_url: String,
    pub error_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl RealtimeSession {
    pub fn transition_to(&mut self, status: RealtimeSessionStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }
}
