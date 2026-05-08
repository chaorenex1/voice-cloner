use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::{
    app::{error::ApiResult, state::AppState},
    audio::{
        device_manager::{AudioDeviceInfo, DefaultAudioDevices},
        preview_player::VoicePreviewState,
    },
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
