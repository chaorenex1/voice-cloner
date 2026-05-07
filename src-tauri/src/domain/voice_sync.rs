use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::voice::SyncStatus;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoiceSyncOperation {
    FullSync,
    Register,
    Update,
    Delete,
    Refresh,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSyncEndpointSet {
    pub sync_url: String,
    pub register_url: String,
    pub update_url: String,
    pub delete_url: String,
    pub refresh_url: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSyncReport {
    pub operation: VoiceSyncOperation,
    pub trace_id: String,
    pub endpoint_url: String,
    pub voice_name: Option<String>,
    pub local_voice_count: usize,
    pub sync_status: Option<SyncStatus>,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSyncState {
    pub reports: Vec<VoiceSyncReport>,
}
