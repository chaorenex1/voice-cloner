use serde::Deserialize;
use tauri::State;

use crate::{
    app::{
        error::{ApiError, ApiResult},
        state::AppState,
    },
    domain::voice_sync::VoiceSyncReport,
    services::voice_sync_manager::parse_incremental_operation,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailVoiceSyncRequest {
    pub operation: String,
    pub voice_name: String,
    pub message: String,
}

#[tauri::command]
pub fn sync_voices_full(state: State<'_, AppState>) -> ApiResult<VoiceSyncReport> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    state
        .voice_sync()
        .full_sync(state.voice_library(), &settings)
        .map_err(Into::into)
}

#[tauri::command]
pub fn register_custom_voice(state: State<'_, AppState>, voice_name: String) -> ApiResult<VoiceSyncReport> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    state
        .voice_sync()
        .register_voice(&voice_name, state.voice_library(), &settings)
        .map_err(Into::into)
}

#[tauri::command]
pub fn update_custom_voice_sync(state: State<'_, AppState>, voice_name: String) -> ApiResult<VoiceSyncReport> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    state
        .voice_sync()
        .update_voice(&voice_name, state.voice_library(), &settings)
        .map_err(Into::into)
}

#[tauri::command]
pub fn delete_custom_voice_sync(state: State<'_, AppState>, voice_name: String) -> ApiResult<VoiceSyncReport> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    state
        .voice_sync()
        .delete_voice(&voice_name, state.voice_library(), &settings)
        .map_err(Into::into)
}

#[tauri::command]
pub fn refresh_voice_runtime(state: State<'_, AppState>) -> ApiResult<VoiceSyncReport> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    state
        .voice_sync()
        .refresh_runtime(state.voice_library(), &settings)
        .map_err(Into::into)
}

#[tauri::command]
pub fn fail_voice_sync(state: State<'_, AppState>, request: FailVoiceSyncRequest) -> ApiResult<VoiceSyncReport> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    let operation = parse_incremental_operation(&request.operation).map_err(ApiError::from)?;
    state
        .voice_sync()
        .mark_voice_sync_failed(
            operation,
            &request.voice_name,
            request.message,
            state.voice_library(),
            &settings,
        )
        .map_err(Into::into)
}

#[tauri::command]
pub fn list_voice_sync_reports(state: State<'_, AppState>) -> Vec<VoiceSyncReport> {
    state.voice_sync().list_reports()
}
