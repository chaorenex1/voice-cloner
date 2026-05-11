use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoiceSeparationSourceType {
    Video,
    Audio,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoiceSeparationStatus {
    Queued,
    ExtractingAudio,
    Decoding,
    Separating,
    MixingNoVocals,
    PostProcessing,
    Ready,
    SavingVoice,
    Saved,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoiceSeparationModel {
    HtDemucs,
    HtDemucsFt,
}

impl VoiceSeparationModel {
    pub fn as_demucs_model(&self) -> &'static str {
        match self {
            Self::HtDemucs => "htdemucs",
            Self::HtDemucsFt => "htdemucs_ft",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoiceSeparationStem {
    Vocals,
    NoVocals,
    Drums,
    Bass,
    Other,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSeparationStems {
    pub vocals: Option<String>,
    pub no_vocals: Option<String>,
    pub drums: Option<String>,
    pub bass: Option<String>,
    pub other: Option<String>,
}

impl VoiceSeparationStems {
    pub fn path_for(&self, stem: &VoiceSeparationStem) -> Option<&str> {
        match stem {
            VoiceSeparationStem::Vocals => self.vocals.as_deref(),
            VoiceSeparationStem::NoVocals => self.no_vocals.as_deref(),
            VoiceSeparationStem::Drums => self.drums.as_deref(),
            VoiceSeparationStem::Bass => self.bass.as_deref(),
            VoiceSeparationStem::Other => self.other.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DenoiseMode {
    Off,
    Standard,
    Strong,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AudioChannelMode {
    Mono,
    Stereo,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoicePostProcessConfig {
    pub trim_silence: bool,
    pub denoise_mode: DenoiseMode,
    pub target_sample_rate: u32,
    pub channels: AudioChannelMode,
    pub loudness_normalization: bool,
    pub target_lufs: f32,
    pub true_peak_db: f32,
    pub peak_limiter: bool,
}

impl Default for VoicePostProcessConfig {
    fn default() -> Self {
        Self {
            trim_silence: false,
            denoise_mode: DenoiseMode::Standard,
            target_sample_rate: 48_000,
            channels: AudioChannelMode::Mono,
            loudness_normalization: true,
            target_lufs: -18.0,
            true_peak_db: -1.5,
            peak_limiter: true,
        }
    }
}

impl VoicePostProcessConfig {
    pub fn default_stereo_output() -> Self {
        Self {
            channels: AudioChannelMode::Stereo,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioPostProcessReport {
    pub input_duration_seconds: f64,
    pub output_duration_seconds: f64,
    pub input_sample_rate: u32,
    pub output_sample_rate: u32,
    pub input_channels: u16,
    pub output_channels: u16,
    pub denoise_applied: bool,
    pub trim_applied: bool,
    pub loudness_applied: bool,
    pub peak_db: f32,
    pub rms_db: f32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSeparationJob {
    pub job_id: String,
    pub trace_id: String,
    pub source_type: VoiceSeparationSourceType,
    pub source_path: String,
    pub source_file_name: String,
    pub model: VoiceSeparationModel,
    pub status: VoiceSeparationStatus,
    pub progress: f32,
    pub current_stage_message: String,
    pub decoded_audio_path: Option<String>,
    pub stems: Option<VoiceSeparationStems>,
    pub post_processed_vocals_path: Option<String>,
    pub post_process_report: Option<AudioPostProcessReport>,
    pub reference_text: Option<String>,
    pub voice_name: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl VoiceSeparationJob {
    pub fn transition_to(&mut self, status: VoiceSeparationStatus, progress: f32, message: impl Into<String>) {
        self.status = status;
        self.progress = progress.clamp(0.0, 1.0);
        self.current_stage_message = message.into();
        self.updated_at = Utc::now();
    }

    pub fn fail(&mut self, message: impl Into<String>) {
        self.status = VoiceSeparationStatus::Failed;
        self.progress = 1.0;
        self.current_stage_message = "人声分离失败".into();
        self.error_message = Some(message.into());
        self.updated_at = Utc::now();
    }
}
