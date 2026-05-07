use tauri::State;

use crate::{
    app::{
        error::{ApiError, ApiResult},
        state::AppState,
    },
    domain::offline_job::OfflineJob,
    services::offline_job_manager::{
        CompleteOfflineJobRequest, CreateOfflineAudioJobRequest, CreateOfflineTextJobRequest, FailOfflineJobRequest,
    },
};

#[tauri::command]
pub fn create_offline_audio_job(
    state: State<'_, AppState>,
    request: CreateOfflineAudioJobRequest,
) -> ApiResult<OfflineJob> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    state
        .offline_jobs()
        .create_audio_job(request, &settings)
        .map_err(Into::into)
}

#[tauri::command]
pub fn create_offline_text_job(
    state: State<'_, AppState>,
    request: CreateOfflineTextJobRequest,
) -> ApiResult<OfflineJob> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    state
        .offline_jobs()
        .create_text_job(request, &settings)
        .map_err(Into::into)
}

#[tauri::command]
pub fn start_offline_job(state: State<'_, AppState>, job_id: String) -> ApiResult<OfflineJob> {
    state.offline_jobs().start_job(&job_id).map_err(Into::into)
}

#[tauri::command]
pub fn cancel_offline_job(state: State<'_, AppState>, job_id: String) -> ApiResult<OfflineJob> {
    state.offline_jobs().cancel_job(&job_id).map_err(Into::into)
}

#[tauri::command]
pub fn retry_offline_job(state: State<'_, AppState>, job_id: String) -> ApiResult<OfflineJob> {
    state.offline_jobs().retry_job(&job_id).map_err(Into::into)
}

#[tauri::command]
pub fn complete_offline_job(
    state: State<'_, AppState>,
    job_id: String,
    request: CompleteOfflineJobRequest,
) -> ApiResult<OfflineJob> {
    state
        .offline_jobs()
        .complete_job(&job_id, request, state.asset_cache())
        .map_err(Into::into)
}

#[tauri::command]
pub fn fail_offline_job(
    state: State<'_, AppState>,
    job_id: String,
    request: FailOfflineJobRequest,
) -> ApiResult<OfflineJob> {
    state.offline_jobs().fail_job(&job_id, request).map_err(Into::into)
}

#[tauri::command]
pub fn get_offline_job(state: State<'_, AppState>, job_id: String) -> ApiResult<OfflineJob> {
    state.offline_jobs().get_job(&job_id).map_err(Into::into)
}

#[tauri::command]
pub fn list_offline_jobs(state: State<'_, AppState>) -> Vec<OfflineJob> {
    state.offline_jobs().list_jobs()
}
