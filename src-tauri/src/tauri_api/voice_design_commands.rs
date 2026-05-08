use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    app::{
        error::{ApiError, ApiResult},
        state::AppState,
    },
    clients::funspeech::offline::transcribe_wav_bytes,
    domain::{voice::CustomVoiceProfile, voice_design::VoiceDesignDraft},
    services::voice_design_manager::{
        CompleteVoiceDesignAsrRequest, CompleteVoiceDesignPreviewRequest, CompleteVoiceInstructionRequest,
        CreateVoiceDesignDraftRequest, FailVoiceDesignDraftRequest, SaveVoiceDesignDraftRequest,
    },
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCustomVoiceProfileRequest {
    pub voice_name: String,
    pub voice_instruction: String,
    pub reference_text: String,
    pub reference_audio_file_name: Option<String>,
    pub reference_audio_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomVoiceProfileView {
    pub voice_name: String,
    pub source_prompt_text: Option<String>,
    pub voice_instruction: String,
    pub reference_text: String,
    pub has_reference_audio: bool,
    pub reference_audio_file_name: Option<String>,
    pub sync_status: crate::domain::voice::SyncStatus,
    pub last_synced_at: Option<chrono::DateTime<Utc>>,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeReferenceAudioRequest {
    pub file_name: String,
    pub audio_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceAudioTranscription {
    pub file_name: String,
    pub text: String,
}

#[tauri::command]
pub fn create_voice_design_draft(
    state: State<'_, AppState>,
    request: CreateVoiceDesignDraftRequest,
) -> ApiResult<VoiceDesignDraft> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    state
        .voice_design()
        .create_draft(request, &settings)
        .map_err(Into::into)
}

#[tauri::command]
pub fn start_voice_design_asr(state: State<'_, AppState>, draft_id: String) -> ApiResult<VoiceDesignDraft> {
    state.voice_design().start_asr(&draft_id).map_err(Into::into)
}

#[tauri::command]
pub fn complete_voice_design_asr(
    state: State<'_, AppState>,
    draft_id: String,
    request: CompleteVoiceDesignAsrRequest,
) -> ApiResult<VoiceDesignDraft> {
    state
        .voice_design()
        .complete_asr(&draft_id, request)
        .map_err(Into::into)
}

#[tauri::command]
pub fn start_voice_design_llm(state: State<'_, AppState>, draft_id: String) -> ApiResult<VoiceDesignDraft> {
    state.voice_design().start_llm(&draft_id).map_err(Into::into)
}

#[tauri::command]
pub fn complete_voice_instruction(
    state: State<'_, AppState>,
    draft_id: String,
    request: CompleteVoiceInstructionRequest,
) -> ApiResult<VoiceDesignDraft> {
    state
        .voice_design()
        .complete_instruction(&draft_id, request)
        .map_err(Into::into)
}

#[tauri::command]
pub fn start_voice_design_generation(state: State<'_, AppState>, draft_id: String) -> ApiResult<VoiceDesignDraft> {
    state.voice_design().start_voice_design(&draft_id).map_err(Into::into)
}

#[tauri::command]
pub fn complete_voice_design_preview(
    state: State<'_, AppState>,
    draft_id: String,
    request: CompleteVoiceDesignPreviewRequest,
) -> ApiResult<VoiceDesignDraft> {
    state
        .voice_design()
        .complete_preview(&draft_id, request, state.asset_cache())
        .map_err(Into::into)
}

#[tauri::command]
pub fn save_voice_design_draft(
    state: State<'_, AppState>,
    draft_id: String,
    request: SaveVoiceDesignDraftRequest,
) -> ApiResult<CustomVoiceProfile> {
    state
        .voice_design()
        .save_custom_voice(&draft_id, request, state.voice_library())
        .map_err(Into::into)
}

#[tauri::command]
pub fn transcribe_reference_audio(
    state: State<'_, AppState>,
    request: TranscribeReferenceAudioRequest,
) -> ApiResult<ReferenceAudioTranscription> {
    if !request.file_name.to_lowercase().ends_with(".wav") {
        return Err(ApiError::from(crate::app::error::AppError::offline_job(
            "reference audio ASR only supports wav files",
        )));
    }

    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    let text = transcribe_wav_bytes(&settings.backend.asr, &request.audio_bytes).map_err(ApiError::from)?;
    Ok(ReferenceAudioTranscription {
        file_name: request.file_name,
        text,
    })
}

#[tauri::command]
pub fn fail_voice_design_draft(
    state: State<'_, AppState>,
    draft_id: String,
    request: FailVoiceDesignDraftRequest,
) -> ApiResult<VoiceDesignDraft> {
    state.voice_design().fail_draft(&draft_id, request).map_err(Into::into)
}

#[tauri::command]
pub fn get_voice_design_draft(state: State<'_, AppState>, draft_id: String) -> ApiResult<VoiceDesignDraft> {
    state.voice_design().get_draft(&draft_id).map_err(Into::into)
}

#[tauri::command]
pub fn list_voice_design_drafts(state: State<'_, AppState>) -> Vec<VoiceDesignDraft> {
    state.voice_design().list_drafts()
}

#[tauri::command]
pub fn list_custom_voices(state: State<'_, AppState>) -> ApiResult<Vec<CustomVoiceProfileView>> {
    state
        .voice_library()
        .list_custom_voices()
        .map(|profiles| profiles.into_iter().map(profile_view).collect())
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_custom_voice(state: State<'_, AppState>, voice_name: String) -> ApiResult<CustomVoiceProfileView> {
    state
        .voice_library()
        .get_custom_voice(&voice_name)
        .map(profile_view)
        .map_err(Into::into)
}

#[tauri::command]
pub fn save_custom_voice_profile(
    state: State<'_, AppState>,
    request: SaveCustomVoiceProfileRequest,
) -> ApiResult<CustomVoiceProfileView> {
    let wav_upload = request.reference_audio_bytes.as_deref().map(|bytes| {
        (
            request.reference_audio_file_name.as_deref().unwrap_or("reference.wav"),
            bytes,
        )
    });

    state
        .voice_library()
        .save_custom_voice_fields(
            &request.voice_name,
            request.voice_instruction,
            request.reference_text,
            wav_upload,
        )
        .map(profile_view)
        .map_err(Into::into)
}

fn profile_view(profile: CustomVoiceProfile) -> CustomVoiceProfileView {
    let reference_audio_file_name = std::path::Path::new(&profile.reference_audio_path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned);
    CustomVoiceProfileView {
        voice_name: profile.voice_name,
        source_prompt_text: profile.source_prompt_text,
        voice_instruction: profile.voice_instruction,
        reference_text: profile.reference_text,
        has_reference_audio: !profile.reference_audio_path.trim().is_empty(),
        reference_audio_file_name,
        sync_status: profile.sync_status,
        last_synced_at: profile.last_synced_at,
        created_at: profile.created_at,
    }
}
