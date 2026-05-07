use tauri::State;

use crate::{
    app::{
        error::{ApiError, ApiResult},
        state::AppState,
    },
    domain::{voice::CustomVoiceProfile, voice_design::VoiceDesignDraft},
    services::voice_design_manager::{
        CompleteVoiceDesignAsrRequest, CompleteVoiceDesignPreviewRequest, CompleteVoiceInstructionRequest,
        CreateVoiceDesignDraftRequest, FailVoiceDesignDraftRequest, SaveVoiceDesignDraftRequest,
    },
};

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
pub fn list_custom_voices(state: State<'_, AppState>) -> ApiResult<Vec<CustomVoiceProfile>> {
    state.voice_library().list_custom_voices().map_err(Into::into)
}

#[tauri::command]
pub fn get_custom_voice(state: State<'_, AppState>, voice_name: String) -> ApiResult<CustomVoiceProfile> {
    state.voice_library().get_custom_voice(&voice_name).map_err(Into::into)
}
