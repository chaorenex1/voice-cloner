use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoiceDesignInputType {
    Text,
    Audio,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoiceDesignStatus {
    Draft,
    AsrRunning,
    AsrCompleted,
    LlmRunning,
    InstructionReady,
    VoiceDesignRunning,
    PreviewReady,
    Saving,
    Saved,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoiceDesignFailureStage {
    Input,
    Asr,
    Llm,
    VoiceDesign,
    Save,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDesignDraft {
    pub draft_id: String,
    pub trace_id: String,
    pub input_type: VoiceDesignInputType,
    pub source_prompt_text: Option<String>,
    pub source_audio_path: Option<String>,
    pub asr_text: Option<String>,
    pub voice_instruction: Option<String>,
    pub reference_text: Option<String>,
    pub reference_audio_path: Option<String>,
    pub voice_name: Option<String>,
    pub status: VoiceDesignStatus,
    pub failure_stage: Option<VoiceDesignFailureStage>,
    pub error_summary: Option<String>,
    pub asr_endpoint: Option<String>,
    pub llm_endpoint: String,
    pub voice_design_endpoint: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl VoiceDesignDraft {
    pub fn transition_to(&mut self, status: VoiceDesignStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }
}
