use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::{
    app::{
        error::{ApiError, ApiResult},
        state::AppState,
    },
    audio::preview_player::VoicePreviewState,
    clients::funspeech::offline::transcribe_wav_bytes,
    domain::{
        voice::CustomVoiceProfile,
        voice_separation::{VoicePostProcessConfig, VoiceSeparationJob, VoiceSeparationStem},
    },
    services::voice_separation_manager::{
        CreateVoiceSeparationJobRequest, SaveSeparatedVocalsRequest, VoiceSeparationDownloadResult,
        VoiceSeparationMutationResult, VoiceSeparationRuntimeStatus,
    },
};

const VOICE_SEPARATION_JOB_UPDATED_EVENT: &str = "voice-separation-job-updated";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartVoiceSeparationJobRequest {
    pub post_process_config: Option<VoicePostProcessConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSeparationStemPreviewRequest {
    pub stem: VoiceSeparationStem,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadVoiceSeparationStemRequest {
    pub stem: VoiceSeparationStem,
    pub target_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSeparationPreviewState {
    pub playing_job_id: Option<String>,
    pub playing_stem: Option<VoiceSeparationStem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceAudioTranscription {
    pub file_name: String,
    pub text: String,
}

#[tauri::command]
pub fn check_voice_separation_runtime(state: State<'_, AppState>) -> VoiceSeparationRuntimeStatus {
    state.voice_separation().runtime_status()
}

#[tauri::command]
pub fn create_voice_separation_job(
    state: State<'_, AppState>,
    request: CreateVoiceSeparationJobRequest,
) -> ApiResult<VoiceSeparationJob> {
    state.voice_separation().create_job(request).map_err(Into::into)
}

#[tauri::command]
pub fn start_voice_separation_job(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
    request: Option<StartVoiceSeparationJobRequest>,
) -> ApiResult<VoiceSeparationJob> {
    let existing = state.voice_separation().get_job(&job_id).map_err(ApiError::from)?;
    let app_state = state.inner().clone();
    let app_handle = app.clone();
    let config = request.and_then(|request| request.post_process_config);
    std::thread::Builder::new()
        .name(format!("voice-separation-{job_id}"))
        .spawn(move || {
            let result = app_state.voice_separation().start_job(&job_id, config);
            match result {
                Ok(job) => {
                    let _ = app_handle.emit(VOICE_SEPARATION_JOB_UPDATED_EVENT, job);
                }
                Err(_) => {
                    if let Ok(job) = app_state.voice_separation().get_job(&job_id) {
                        let _ = app_handle.emit(VOICE_SEPARATION_JOB_UPDATED_EVENT, job);
                    }
                }
            }
        })
        .map_err(|source| {
            ApiError::from(crate::app::error::AppError::io(
                "starting voice separation worker",
                source,
            ))
        })?;
    Ok(existing)
}

#[tauri::command]
pub fn get_voice_separation_job(state: State<'_, AppState>, job_id: String) -> ApiResult<VoiceSeparationJob> {
    state.voice_separation().get_job(&job_id).map_err(Into::into)
}

#[tauri::command]
pub fn list_voice_separation_jobs(state: State<'_, AppState>) -> Vec<VoiceSeparationJob> {
    state.voice_separation().list_jobs()
}

#[tauri::command]
pub fn cancel_voice_separation_job(state: State<'_, AppState>, job_id: String) -> ApiResult<VoiceSeparationJob> {
    state.voice_separation().cancel_job(&job_id).map_err(Into::into)
}

#[tauri::command]
pub fn delete_voice_separation_job(
    state: State<'_, AppState>,
    job_id: String,
) -> ApiResult<VoiceSeparationMutationResult> {
    let _ = state.voice_preview().stop();
    state.voice_separation().delete_job(&job_id).map_err(Into::into)
}

#[tauri::command]
pub fn preview_voice_separation_stem(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
    request: VoiceSeparationStemPreviewRequest,
) -> ApiResult<VoiceSeparationPreviewState> {
    let path = state
        .voice_separation()
        .stem_path(&job_id, &request.stem)
        .map_err(ApiError::from)?;
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    let device = state
        .audio_devices()
        .output_device_by_id(settings.device.output_device_id.as_deref())
        .map_err(ApiError::from)?;
    let preview_key = separation_preview_key(&job_id, &request.stem);
    let playback = state
        .voice_preview()
        .toggle(preview_key, path, device, move |_finished_key| {
            let _ = app.emit(
                "voice-separation-preview-finished",
                VoiceSeparationPreviewState {
                    playing_job_id: None,
                    playing_stem: None,
                },
            );
        })
        .map_err(ApiError::from)?;
    Ok(preview_state_from_key(playback, &job_id, &request.stem))
}

#[tauri::command]
pub fn stop_voice_separation_preview(state: State<'_, AppState>) -> VoiceSeparationPreviewState {
    let _ = state.voice_preview().stop();
    VoiceSeparationPreviewState {
        playing_job_id: None,
        playing_stem: None,
    }
}

#[tauri::command]
pub fn download_voice_separation_stem(
    state: State<'_, AppState>,
    job_id: String,
    request: DownloadVoiceSeparationStemRequest,
) -> ApiResult<VoiceSeparationDownloadResult> {
    state
        .voice_separation()
        .copy_stem_to(&job_id, &request.stem, request.target_path)
        .map_err(Into::into)
}

#[tauri::command]
pub fn transcribe_separated_vocals(
    state: State<'_, AppState>,
    job_id: String,
) -> ApiResult<ReferenceAudioTranscription> {
    let vocals_path = state
        .voice_separation()
        .processed_vocals_path(&job_id)
        .map_err(ApiError::from)?;
    let bytes = std::fs::read(&vocals_path).map_err(|source| {
        ApiError::from(crate::app::error::AppError::io(
            "reading separated vocals for ASR",
            source,
        ))
    })?;
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    let text = transcribe_wav_bytes(&settings.backend.asr, &bytes).map_err(ApiError::from)?;
    state
        .voice_separation()
        .mark_reference_text(&job_id, text.clone())
        .map_err(ApiError::from)?;
    Ok(ReferenceAudioTranscription {
        file_name: vocals_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("vocals.wav")
            .to_string(),
        text,
    })
}

#[tauri::command]
pub fn save_separated_vocals_as_custom_voice(
    state: State<'_, AppState>,
    job_id: String,
    request: SaveSeparatedVocalsRequest,
) -> ApiResult<CustomVoiceProfile> {
    state
        .voice_separation()
        .save_as_custom_voice(&job_id, request, state.voice_library())
        .map_err(Into::into)
}

fn separation_preview_key(job_id: &str, stem: &VoiceSeparationStem) -> String {
    format!("voice-separation:{job_id}:{stem:?}")
}

fn preview_state_from_key(
    playback: VoicePreviewState,
    job_id: &str,
    stem: &VoiceSeparationStem,
) -> VoiceSeparationPreviewState {
    if playback.playing_voice_name.is_some() {
        VoiceSeparationPreviewState {
            playing_job_id: Some(job_id.to_string()),
            playing_stem: Some(stem.clone()),
        }
    } else {
        VoiceSeparationPreviewState {
            playing_job_id: None,
            playing_stem: None,
        }
    }
}
