use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::runtime_params::RuntimeParams;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoicePreset {
    pub name: String,
    pub description: String,
    pub preview_audio_url: Option<String>,
    pub reference_text: Option<String>,
    pub default_params: RuntimeParams,
    pub backend_binding: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SyncStatus {
    LocalOnly,
    PendingSync,
    Synced,
    Failed,
    Conflict,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomVoiceProfile {
    pub voice_name: String,
    pub source_prompt_text: Option<String>,
    pub asr_text: Option<String>,
    pub voice_instruction: String,
    pub reference_audio_path: String,
    pub reference_text: String,
    pub sync_status: SyncStatus,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
