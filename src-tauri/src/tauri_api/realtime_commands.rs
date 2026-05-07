use tauri::State;

use crate::{
    app::{
        error::{ApiError, ApiResult},
        state::AppState,
    },
    audio::{
        frame::{PcmFormat, SampleFormat},
        virtual_mic::VirtualMicAdapter,
    },
    domain::session::RealtimeSession,
    services::session_manager::{
        CreateRealtimeSessionRequest, SwitchRealtimeVoiceRequest, UpdateRealtimeParamsRequest,
    },
    services::realtime_stream_manager::RealtimeStreamSnapshot,
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
pub async fn start_realtime_session(state: State<'_, AppState>, session_id: String) -> ApiResult<RealtimeSession> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    let format = PcmFormat {
        sample_rate: settings.runtime.default_sample_rate,
        frame_ms: settings.runtime.audio_frame_ms,
        sample_format: SampleFormat::I16,
        ..PcmFormat::default()
    };
    if settings.device.virtual_mic_enabled {
        state
            .virtual_mic()
            .set_target_device_id(settings.device.virtual_mic_device_id.clone());
        state.virtual_mic().start(format).map_err(ApiError::from)?;
    }

    let started = state
        .sessions()
        .start_realtime_session(&session_id, state.audio_engine())
        .map_err(ApiError::from);
    if started.is_err() && settings.device.virtual_mic_enabled {
        let _ = state.virtual_mic().stop();
    }
    let session = started?;
    let input_device = match state
        .audio_devices()
        .input_device_by_id(settings.device.input_device_id.as_deref())
    {
        Ok(device) => device,
        Err(error) => {
            let _ = state.sessions().mark_realtime_session_failed(
                &session_id,
                error.to_string(),
                state.audio_engine(),
            );
            let _ = state.virtual_mic().stop();
            return Err(ApiError::from(error));
        }
    };
    if let Err(error) = state
        .realtime_streams()
        .start(
            session.clone(),
            format,
            Some(input_device),
            state.virtual_mic_handle(),
            settings.device.virtual_mic_enabled,
        )
        .await
    {
        let _ = state.sessions().mark_realtime_session_failed(
            &session_id,
            error.to_string(),
            state.audio_engine(),
        );
        let _ = state.virtual_mic().stop();
        return Err(ApiError::from(error));
    }
    Ok(session)
}

#[tauri::command]
pub async fn stop_realtime_session(state: State<'_, AppState>, session_id: String) -> ApiResult<RealtimeSession> {
    let _ = state.realtime_streams().stop(&session_id).await;
    let stopped = state
        .sessions()
        .stop_realtime_session(&session_id, state.audio_engine())
        .map_err(ApiError::from);
    let _ = state.virtual_mic().stop();
    stopped
}

#[tauri::command]
pub async fn fail_realtime_session(
    state: State<'_, AppState>,
    session_id: String,
    message: String,
) -> ApiResult<RealtimeSession> {
    let _ = state.realtime_streams().stop(&session_id).await;
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
        .inspect(|session| {
            let _ = state
                .realtime_streams()
                .update_params(&session_id, session.runtime_params.clone());
        })
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
        .inspect(|session| {
            let _ = state
                .realtime_streams()
                .switch_voice(&session_id, session.voice_name.clone());
        })
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

#[tauri::command]
pub fn get_realtime_stream_snapshot(
    state: State<'_, AppState>,
    session_id: String,
) -> ApiResult<RealtimeStreamSnapshot> {
    state
        .realtime_streams()
        .get_snapshot(&session_id)
        .map_err(Into::into)
}

#[tauri::command]
pub fn list_realtime_stream_snapshots(state: State<'_, AppState>) -> Vec<RealtimeStreamSnapshot> {
    state.realtime_streams().list_snapshots()
}
