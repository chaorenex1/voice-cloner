use std::{collections::BTreeMap, path::PathBuf, sync::RwLock};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    app::{
        error::{AppError, AppResult},
        trace::{new_entity_id, TraceId},
    },
    clients::{
        funspeech::{offline::OfflineEndpoints, voice_design::VoiceDesignEndpoint},
        local_llm::LocalLlmEndpoint,
    },
    domain::{
        settings::AppSettings,
        voice::{CustomVoiceProfile, SyncStatus},
        voice_design::{VoiceDesignDraft, VoiceDesignFailureStage, VoiceDesignInputType, VoiceDesignStatus},
    },
    services::{asset_cache::AssetCache, voice_library::VoiceLibrary},
};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateVoiceDesignDraftRequest {
    pub input_type: VoiceDesignInputType,
    pub source_prompt_text: Option<String>,
    pub source_audio_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompleteVoiceDesignAsrRequest {
    pub asr_text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompleteVoiceInstructionRequest {
    pub voice_instruction: String,
    pub reference_text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompleteVoiceDesignPreviewRequest {
    pub reference_audio_path: Option<String>,
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveVoiceDesignDraftRequest {
    pub voice_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FailVoiceDesignDraftRequest {
    pub stage: VoiceDesignFailureStage,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct VoiceDesignManager {
    drafts: RwLock<BTreeMap<String, VoiceDesignDraft>>,
}

impl VoiceDesignManager {
    pub fn create_draft(
        &self,
        request: CreateVoiceDesignDraftRequest,
        settings: &AppSettings,
    ) -> AppResult<VoiceDesignDraft> {
        settings.validate().map_err(AppError::invalid_settings)?;
        validate_input(&request)?;
        let offline = OfflineEndpoints::from_backend_configs(&settings.backend.asr, &settings.backend.tts);
        let llm = LocalLlmEndpoint::from_backend_config(&settings.backend.llm);
        let voice_design = VoiceDesignEndpoint::from_backend_config(&settings.backend.tts);
        let now = Utc::now();
        let draft = VoiceDesignDraft {
            draft_id: new_entity_id("voice-design"),
            trace_id: TraceId::new("voice-design").into_string(),
            input_type: request.input_type,
            source_prompt_text: request.source_prompt_text.map(|text| text.trim().to_string()),
            source_audio_path: request.source_audio_path.map(|path| path.trim().to_string()),
            asr_text: None,
            voice_instruction: None,
            reference_text: None,
            reference_audio_path: None,
            voice_name: None,
            status: VoiceDesignStatus::Draft,
            failure_stage: None,
            error_summary: None,
            asr_endpoint: Some(offline.asr_url),
            llm_endpoint: llm.generate_url,
            voice_design_endpoint: voice_design.voice_design_url,
            created_at: now,
            updated_at: now,
        };

        self.drafts
            .write()
            .expect("voice design manager lock poisoned")
            .insert(draft.draft_id.clone(), draft.clone());
        Ok(draft)
    }

    pub fn start_asr(&self, draft_id: &str) -> AppResult<VoiceDesignDraft> {
        self.with_draft(draft_id, |draft| {
            if draft.input_type != VoiceDesignInputType::Audio {
                return Err(AppError::offline_job(
                    "ASR can only start for audio voice design drafts",
                ));
            }
            draft.failure_stage = None;
            draft.error_summary = None;
            draft.transition_to(VoiceDesignStatus::AsrRunning);
            Ok(draft.clone())
        })
    }

    pub fn complete_asr(&self, draft_id: &str, request: CompleteVoiceDesignAsrRequest) -> AppResult<VoiceDesignDraft> {
        let asr_text = require_non_empty("asrText", request.asr_text)?;
        self.with_draft(draft_id, |draft| {
            draft.asr_text = Some(asr_text);
            draft.transition_to(VoiceDesignStatus::AsrCompleted);
            Ok(draft.clone())
        })
    }

    pub fn start_llm(&self, draft_id: &str) -> AppResult<VoiceDesignDraft> {
        self.with_draft(draft_id, |draft| {
            let prompt = draft.asr_text.as_ref().or(draft.source_prompt_text.as_ref());
            if prompt.map(|value| value.trim().is_empty()).unwrap_or(true) {
                return Err(AppError::offline_job(
                    "voice design prompt text is required before LLM generation",
                ));
            }
            draft.failure_stage = None;
            draft.error_summary = None;
            draft.transition_to(VoiceDesignStatus::LlmRunning);
            Ok(draft.clone())
        })
    }

    pub fn complete_instruction(
        &self,
        draft_id: &str,
        request: CompleteVoiceInstructionRequest,
    ) -> AppResult<VoiceDesignDraft> {
        let voice_instruction = require_non_empty("voiceInstruction", request.voice_instruction)?;
        let reference_text = require_non_empty("referenceText", request.reference_text)?;
        self.with_draft(draft_id, |draft| {
            draft.voice_instruction = Some(voice_instruction);
            draft.reference_text = Some(reference_text);
            draft.transition_to(VoiceDesignStatus::InstructionReady);
            Ok(draft.clone())
        })
    }

    pub fn start_voice_design(&self, draft_id: &str) -> AppResult<VoiceDesignDraft> {
        self.with_draft(draft_id, |draft| {
            if draft
                .voice_instruction
                .as_ref()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(AppError::offline_job(
                    "voiceInstruction is required before reference audio generation",
                ));
            }
            if draft
                .reference_text
                .as_ref()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(AppError::offline_job(
                    "referenceText is required before reference audio generation",
                ));
            }
            draft.failure_stage = None;
            draft.error_summary = None;
            draft.transition_to(VoiceDesignStatus::VoiceDesignRunning);
            Ok(draft.clone())
        })
    }

    pub fn complete_preview(
        &self,
        draft_id: &str,
        request: CompleteVoiceDesignPreviewRequest,
        cache: &AssetCache,
    ) -> AppResult<VoiceDesignDraft> {
        self.with_draft(draft_id, |draft| {
            let format = request.output_format.as_deref().unwrap_or("wav");
            let artifact_path = if let Some(path) = request.reference_audio_path {
                cache
                    .register_voice_design_artifact(draft_id, format, PathBuf::from(path))?
                    .path
            } else {
                cache.voice_design_artifact_path(draft_id, format)?.path
            };
            draft.reference_audio_path = Some(artifact_path.to_string_lossy().into_owned());
            draft.transition_to(VoiceDesignStatus::PreviewReady);
            Ok(draft.clone())
        })
    }

    pub fn save_custom_voice(
        &self,
        draft_id: &str,
        request: SaveVoiceDesignDraftRequest,
        library: &VoiceLibrary,
    ) -> AppResult<CustomVoiceProfile> {
        let voice_name = require_non_empty("voiceName", request.voice_name)?;
        let mut drafts = self.drafts.write().expect("voice design manager lock poisoned");
        let draft = drafts
            .get_mut(draft_id)
            .ok_or_else(|| AppError::offline_job(format!("voice design draft not found: {draft_id}")))?;

        draft.transition_to(VoiceDesignStatus::Saving);
        let profile = CustomVoiceProfile {
            voice_name: voice_name.clone(),
            source_prompt_text: draft.source_prompt_text.clone(),
            asr_text: draft.asr_text.clone(),
            voice_instruction: draft
                .voice_instruction
                .clone()
                .ok_or_else(|| AppError::offline_job("voiceInstruction is required before saving custom voice"))?,
            reference_audio_path: draft
                .reference_audio_path
                .clone()
                .ok_or_else(|| AppError::offline_job("referenceAudioPath is required before saving custom voice"))?,
            reference_text: draft
                .reference_text
                .clone()
                .ok_or_else(|| AppError::offline_job("referenceText is required before saving custom voice"))?,
            sync_status: SyncStatus::PendingSync,
            last_synced_at: None,
            created_at: Utc::now(),
        };
        let saved = library.save_custom_voice(profile)?;
        draft.voice_name = Some(voice_name);
        draft.transition_to(VoiceDesignStatus::Saved);
        Ok(saved)
    }

    pub fn fail_draft(&self, draft_id: &str, request: FailVoiceDesignDraftRequest) -> AppResult<VoiceDesignDraft> {
        let message = require_non_empty("message", request.message)?;
        self.with_draft(draft_id, |draft| {
            draft.failure_stage = Some(request.stage);
            draft.error_summary = Some(message);
            draft.transition_to(VoiceDesignStatus::Failed);
            Ok(draft.clone())
        })
    }

    pub fn get_draft(&self, draft_id: &str) -> AppResult<VoiceDesignDraft> {
        self.drafts
            .read()
            .expect("voice design manager lock poisoned")
            .get(draft_id)
            .cloned()
            .ok_or_else(|| AppError::offline_job(format!("voice design draft not found: {draft_id}")))
    }

    pub fn list_drafts(&self) -> Vec<VoiceDesignDraft> {
        self.drafts
            .read()
            .expect("voice design manager lock poisoned")
            .values()
            .cloned()
            .collect()
    }

    fn with_draft<T>(
        &self,
        draft_id: &str,
        change: impl FnOnce(&mut VoiceDesignDraft) -> AppResult<T>,
    ) -> AppResult<T> {
        let mut drafts = self.drafts.write().expect("voice design manager lock poisoned");
        let draft = drafts
            .get_mut(draft_id)
            .ok_or_else(|| AppError::offline_job(format!("voice design draft not found: {draft_id}")))?;
        change(draft)
    }
}

fn validate_input(request: &CreateVoiceDesignDraftRequest) -> AppResult<()> {
    match request.input_type {
        VoiceDesignInputType::Text => {
            require_non_empty(
                "sourcePromptText",
                request.source_prompt_text.clone().unwrap_or_default(),
            )?;
        }
        VoiceDesignInputType::Audio => {
            require_non_empty("sourceAudioPath", request.source_audio_path.clone().unwrap_or_default())?;
        }
    }
    Ok(())
}

fn require_non_empty(field: &str, value: String) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::offline_job(format!("{field} is required")))
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{
        domain::{
            settings::AppSettings,
            voice::SyncStatus,
            voice_design::{VoiceDesignFailureStage, VoiceDesignInputType, VoiceDesignStatus},
        },
        services::{asset_cache::AssetCache, voice_library::VoiceLibrary},
    };

    use super::{
        CompleteVoiceDesignAsrRequest, CompleteVoiceDesignPreviewRequest, CompleteVoiceInstructionRequest,
        CreateVoiceDesignDraftRequest, FailVoiceDesignDraftRequest, SaveVoiceDesignDraftRequest, VoiceDesignManager,
    };

    fn temp_root(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("voice-cloner-{name}-{unique}"))
    }

    fn cache() -> AssetCache {
        let root = temp_root("voice-design-cache");
        AssetCache::new(root.join("offline"), root.join("voice-design")).unwrap()
    }

    fn library() -> VoiceLibrary {
        VoiceLibrary::new(temp_root("voice-library")).unwrap()
    }

    fn wav_bytes(samples: &[f32]) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
            for sample in samples {
                writer.write_sample((sample * i16::MAX as f32) as i16).unwrap();
            }
            writer.finalize().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn voice_design_manager_completes_text_prompt_flow_and_saves_profile() {
        let manager = VoiceDesignManager::default();
        let cache = cache();
        let library = library();
        let draft = manager
            .create_draft(
                CreateVoiceDesignDraftRequest {
                    input_type: VoiceDesignInputType::Text,
                    source_prompt_text: Some("warm narrator".into()),
                    source_audio_path: None,
                },
                &AppSettings::default(),
            )
            .unwrap();

        assert_eq!(draft.status, VoiceDesignStatus::Draft);
        assert!(draft.llm_endpoint.ends_with("/api/generate"));
        assert!(draft.voice_design_endpoint.ends_with("/voices/v1/voice-design"));

        let llm = manager.start_llm(&draft.draft_id).unwrap();
        assert_eq!(llm.status, VoiceDesignStatus::LlmRunning);
        manager
            .complete_instruction(
                &draft.draft_id,
                CompleteVoiceInstructionRequest {
                    voice_instruction: "warm, close, calm".into(),
                    reference_text: "Hello from the studio.".into(),
                },
            )
            .unwrap();
        manager.start_voice_design(&draft.draft_id).unwrap();
        let preview = manager
            .complete_preview(
                &draft.draft_id,
                CompleteVoiceDesignPreviewRequest {
                    reference_audio_path: None,
                    output_format: Some("wav".into()),
                },
                &cache,
            )
            .unwrap();
        assert_eq!(preview.status, VoiceDesignStatus::PreviewReady);
        let preview_audio_path = preview.reference_audio_path.unwrap();
        assert!(preview_audio_path.ends_with(".wav"));
        std::fs::write(&preview_audio_path, wav_bytes(&[0.2])).unwrap();

        let profile = manager
            .save_custom_voice(
                &draft.draft_id,
                SaveVoiceDesignDraftRequest {
                    voice_name: "warm-narrator".into(),
                },
                &library,
            )
            .unwrap();
        assert_eq!(profile.voice_name, "warm-narrator");
        assert_eq!(profile.sync_status, SyncStatus::PendingSync);
        assert_eq!(
            manager.get_draft(&draft.draft_id).unwrap().status,
            VoiceDesignStatus::Saved
        );
    }

    #[test]
    fn voice_design_manager_supports_audio_asr_stage() {
        let manager = VoiceDesignManager::default();
        let draft = manager
            .create_draft(
                CreateVoiceDesignDraftRequest {
                    input_type: VoiceDesignInputType::Audio,
                    source_prompt_text: None,
                    source_audio_path: Some("C:/recordings/voice.wav".into()),
                },
                &AppSettings::default(),
            )
            .unwrap();

        assert_eq!(
            manager.start_asr(&draft.draft_id).unwrap().status,
            VoiceDesignStatus::AsrRunning
        );
        let asr = manager
            .complete_asr(
                &draft.draft_id,
                CompleteVoiceDesignAsrRequest {
                    asr_text: "make a bright voice".into(),
                },
            )
            .unwrap();

        assert_eq!(asr.status, VoiceDesignStatus::AsrCompleted);
        assert_eq!(asr.asr_text.as_deref(), Some("make a bright voice"));
    }

    #[test]
    fn voice_design_manager_marks_failed_stage() {
        let manager = VoiceDesignManager::default();
        let draft = manager
            .create_draft(
                CreateVoiceDesignDraftRequest {
                    input_type: VoiceDesignInputType::Text,
                    source_prompt_text: Some("warm narrator".into()),
                    source_audio_path: None,
                },
                &AppSettings::default(),
            )
            .unwrap();
        let failed = manager
            .fail_draft(
                &draft.draft_id,
                FailVoiceDesignDraftRequest {
                    stage: VoiceDesignFailureStage::Llm,
                    message: "local model unavailable".into(),
                },
            )
            .unwrap();

        assert_eq!(failed.status, VoiceDesignStatus::Failed);
        assert_eq!(failed.failure_stage, Some(VoiceDesignFailureStage::Llm));
        assert_eq!(failed.error_summary.as_deref(), Some("local model unavailable"));
    }
}
