use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::runtime_params::RuntimeParams;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OfflineJobInputType {
    Audio,
    Text,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OfflineJobStatus {
    Created,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OfflineJob {
    pub job_id: String,
    pub trace_id: String,
    pub input_type: OfflineJobInputType,
    pub input_ref: String,
    pub voice_name: String,
    pub runtime_params: RuntimeParams,
    pub output_format: String,
    pub status: OfflineJobStatus,
    pub artifact_url: Option<String>,
    pub local_artifact_path: Option<String>,
    pub error_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OfflineJob {
    pub fn transition_to(&mut self, status: OfflineJobStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }
}
