use tauri::State;

use crate::{
    app::{
        error::{ApiError, ApiResult},
        state::AppState,
    },
    audio::{frame::PcmFormat, virtual_mic::VirtualMicAdapter},
    domain::session::RealtimeSession,
    services::session_manager::{
        CreateRealtimeSessionRequest, SwitchRealtimeVoiceRequest, UpdateRealtimeParamsRequest,
    },
};

#[tauri::command]
pub fn create_realtime_session(
    state: State<'_, AppState>,
    request: CreateRealtimeSessionRequest,
) -> ApiResult<RealtimeSession> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    state
        .sessions()
        .create_realtime_session(request, &settings)
        .map_err(Into::into)
}

#[tauri::command]
pub fn start_realtime_session(state: State<'_, AppState>, session_id: String) -> ApiResult<RealtimeSession> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    if settings.device.virtual_mic_enabled {
        state
            .virtual_mic()
            .set_target_device_id(settings.device.virtual_mic_device_id.clone());
        state
            .virtual_mic()
            .start(PcmFormat {
                sample_rate: settings.runtime.default_sample_rate,
                frame_ms: settings.runtime.audio_frame_ms,
                ..PcmFormat::default()
            })
            .map_err(ApiError::from)?;
    }

    let started = state
        .sessions()
        .start_realtime_session(&session_id, state.audio_engine())
        .map_err(ApiError::from);
    if started.is_err() && settings.device.virtual_mic_enabled {
        let _ = state.virtual_mic().stop();
    }
    started
}

#[tauri::command]
pub fn stop_realtime_session(state: State<'_, AppState>, session_id: String) -> ApiResult<RealtimeSession> {
    let stopped = state
        .sessions()
        .stop_realtime_session(&session_id, state.audio_engine())
        .map_err(ApiError::from);
    let _ = state.virtual_mic().stop();
    stopped
}

#[tauri::command]
pub fn fail_realtime_session(
    state: State<'_, AppState>,
    session_id: String,
    message: String,
) -> ApiResult<RealtimeSession> {
    let failed = state
        .sessions()
        .mark_realtime_session_failed(&session_id, message, state.audio_engine())
        .map_err(ApiError::from);
    let _ = state.virtual_mic().stop();
    failed
}

#[tauri::command]
pub fn update_realtime_params(
    state: State<'_, AppState>,
    session_id: String,
    request: UpdateRealtimeParamsRequest,
) -> ApiResult<RealtimeSession> {
    state
        .sessions()
        .update_realtime_params(&session_id, request)
        .map_err(Into::into)
}

#[tauri::command]
pub fn switch_realtime_voice(
    state: State<'_, AppState>,
    session_id: String,
    request: SwitchRealtimeVoiceRequest,
) -> ApiResult<RealtimeSession> {
    state
        .sessions()
        .switch_realtime_voice(&session_id, request)
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_realtime_session(state: State<'_, AppState>, session_id: String) -> ApiResult<RealtimeSession> {
    state.sessions().get_realtime_session(&session_id).map_err(Into::into)
}

#[tauri::command]
pub fn list_realtime_sessions(state: State<'_, AppState>) -> Vec<RealtimeSession> {
    state.sessions().list_realtime_sessions()
}
