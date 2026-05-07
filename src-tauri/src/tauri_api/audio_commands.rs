use tauri::State;

use crate::{
    app::{error::ApiResult, state::AppState},
    audio::device_manager::{AudioDeviceInfo, DefaultAudioDevices},
};

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
