use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::{
    app::{error::ApiResult, state::AppState},
    audio::{
        device_manager::{AudioDeviceInfo, DefaultAudioDevices},
        preview_player::VoicePreviewState,
    },
    domain::offline_job::OfflineJobStatus,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoicePreviewRequest {
    pub voice_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoicePreviewFinishedEvent {
    pub voice_name: String,
    pub playing_voice_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineJobPreviewRequest {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineJobPreviewState {
    pub playing_job_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineJobPreviewFinishedEvent {
    pub job_id: String,
    pub playing_job_id: Option<String>,
}

#[tauri::command]
pub fn list_audio_input_devices(state: State<'_, AppState>) -> ApiResult<Vec<AudioDeviceInfo>> {
    state.audio_devices().list_input_devices().map_err(Into::into)
}

#[tauri::command]
pub fn list_audio_output_devices(state: State<'_, AppState>) -> ApiResult<Vec<AudioDeviceInfo>> {
    state.audio_devices().list_output_devices().map_err(Into::into)
}

#[tauri::command]
pub fn get_default_audio_devices(state: State<'_, AppState>) -> ApiResult<DefaultAudioDevices> {
    state.audio_devices().default_devices().map_err(Into::into)
}

#[tauri::command]
pub fn toggle_voice_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    request: VoicePreviewRequest,
) -> ApiResult<VoicePreviewState> {
    let settings = state
        .settings()
        .load_or_default()
        .map_err(crate::app::error::ApiError::from)?;
    let device = state
        .audio_devices()
        .output_device_by_id(settings.device.output_device_id.as_deref())
        .map_err(crate::app::error::ApiError::from)?;
    let reference_audio_path = state
        .voice_library()
        .reference_audio_path_for_voice(&request.voice_name)
        .map_err(crate::app::error::ApiError::from)?;
    state
        .voice_preview()
        .toggle(request.voice_name, reference_audio_path, device, move |voice_name| {
            let _ = app.emit(
                "voice-preview-finished",
                VoicePreviewFinishedEvent {
                    voice_name,
                    playing_voice_name: None,
                },
            );
        })
        .map_err(Into::into)
}

#[tauri::command]
pub fn stop_voice_preview(state: State<'_, AppState>) -> VoicePreviewState {
    state.voice_preview().stop()
}

#[tauri::command]
pub fn toggle_offline_job_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    request: OfflineJobPreviewRequest,
) -> ApiResult<OfflineJobPreviewState> {
    let job = state
        .offline_jobs()
        .get_job(&request.job_id)
        .map_err(crate::app::error::ApiError::from)?;
    if job.status != OfflineJobStatus::Completed {
        return Err(crate::app::error::ApiError::from(
            crate::app::error::AppError::offline_job("only completed offline jobs can be previewed"),
        ));
    }
    let artifact_path = job.local_artifact_path.ok_or_else(|| {
        crate::app::error::ApiError::from(crate::app::error::AppError::offline_job(
            "completed offline job has no local artifact",
        ))
    })?;
    if !artifact_path.to_ascii_lowercase().ends_with(".wav") {
        return Err(crate::app::error::ApiError::from(crate::app::error::AppError::audio(
            "offline job preview only supports wav files",
        )));
    }

    let settings = state
        .settings()
        .load_or_default()
        .map_err(crate::app::error::ApiError::from)?;
    let device = state
        .audio_devices()
        .output_device_by_id(settings.device.output_device_id.as_deref())
        .map_err(crate::app::error::ApiError::from)?;
    let preview_key = offline_preview_key(&job.job_id);
    let playback = state
        .voice_preview()
        .toggle(preview_key, artifact_path, device, move |finished_key| {
            if let Some(job_id) = offline_job_id_from_key(&finished_key) {
                let _ = app.emit(
                    "offline-job-preview-finished",
                    OfflineJobPreviewFinishedEvent {
                        job_id,
                        playing_job_id: None,
                    },
                );
            }
        })
        .map_err(crate::app::error::ApiError::from)?;

    Ok(OfflineJobPreviewState {
        playing_job_id: playback.playing_voice_name.as_deref().and_then(offline_job_id_from_key),
    })
}

#[tauri::command]
pub fn stop_offline_job_preview(state: State<'_, AppState>) -> OfflineJobPreviewState {
    let _ = state.voice_preview().stop();
    OfflineJobPreviewState { playing_job_id: None }
}

fn offline_preview_key(job_id: &str) -> String {
    format!("offline-job:{job_id}")
}

fn offline_job_id_from_key(key: &str) -> Option<String> {
    key.strip_prefix("offline-job:").map(str::to_string)
}
