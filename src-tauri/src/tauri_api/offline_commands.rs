use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOfflineJobRequest {
    pub target_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineJobsClearResult {
    pub removed_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineJobDeleteResult {
    pub removed: OfflineJob,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineJobDownloadResult {
    pub target_path: String,
}

#[tauri::command]
pub fn create_offline_audio_job(
    state: State<'_, AppState>,
    request: CreateOfflineAudioJobRequest,
) -> ApiResult<OfflineJob> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    state
        .offline_jobs()
        .create_audio_job(request, &settings, state.asset_cache())
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

const OFFLINE_JOB_UPDATED_EVENT: &str = "offline-job-updated";

#[tauri::command]
pub fn start_offline_job(app: AppHandle, state: State<'_, AppState>, job_id: String) -> ApiResult<OfflineJob> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    let running = state.offline_jobs().begin_job(&job_id).map_err(ApiError::from)?;
    let _ = app.emit(OFFLINE_JOB_UPDATED_EVENT, running.clone());

    let app_handle = app.clone();
    let app_state = state.inner().clone();
    std::thread::Builder::new()
        .name(format!("offline-job-{job_id}"))
        .spawn(move || {
            let emit_update = |job: OfflineJob| {
                let _ = app_handle.emit(OFFLINE_JOB_UPDATED_EVENT, job);
            };
            let _ = app_state.offline_jobs().run_started_job_with_updates(
                running,
                &settings,
                app_state.asset_cache(),
                emit_update,
            );
        })
        .map_err(|source| ApiError::from(crate::app::error::AppError::io("starting offline job worker", source)))?;

    state.offline_jobs().get_job(&job_id).map_err(Into::into)
}

#[tauri::command]
pub fn cancel_offline_job(state: State<'_, AppState>, job_id: String) -> ApiResult<OfflineJob> {
    state.offline_jobs().cancel_job(&job_id).map_err(Into::into)
}

#[tauri::command]
pub fn retry_offline_job(state: State<'_, AppState>, job_id: String) -> ApiResult<OfflineJob> {
    state
        .offline_jobs()
        .retry_job(&job_id, state.asset_cache())
        .map_err(Into::into)
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

#[tauri::command]
pub fn clear_offline_jobs(state: State<'_, AppState>) -> ApiResult<OfflineJobsClearResult> {
    let _ = state.voice_preview().stop();
    state
        .offline_jobs()
        .clear_jobs(state.asset_cache())
        .map(|removed| OfflineJobsClearResult {
            removed_count: removed.len(),
        })
        .map_err(Into::into)
}

#[tauri::command]
pub fn delete_offline_job(state: State<'_, AppState>, job_id: String) -> ApiResult<OfflineJobDeleteResult> {
    let _ = state.voice_preview().stop();
    state
        .offline_jobs()
        .delete_job(&job_id, state.asset_cache())
        .map(|removed| OfflineJobDeleteResult { removed })
        .map_err(Into::into)
}

#[tauri::command]
pub fn download_offline_job(
    state: State<'_, AppState>,
    job_id: String,
    request: DownloadOfflineJobRequest,
) -> ApiResult<OfflineJobDownloadResult> {
    state
        .offline_jobs()
        .copy_artifact_to(&job_id, request.target_path)
        .map(|target_path| OfflineJobDownloadResult {
            target_path: target_path.to_string_lossy().into_owned(),
        })
        .map_err(Into::into)
}
