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
    services::realtime_stream_manager::{RealtimeStreamMode, RealtimeStreamSnapshot},
    services::session_manager::{
        CreateRealtimeSessionRequest, SwitchRealtimeVoiceRequest, UpdateRealtimeParamsRequest,
    },
};

#[tauri::command]
pub fn create_realtime_session(
    state: State<'_, AppState>,
    request: CreateRealtimeSessionRequest,
) -> ApiResult<RealtimeSession> {
    tracing::debug!(voice_name = %request.voice_name, "create realtime session requested");
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    let session = state
        .sessions()
        .create_realtime_session(request, &settings)
        .map_err(ApiError::from)?;
    tracing::debug!(
        session_id = %session.session_id,
        trace_id = %session.trace_id,
        voice_name = %session.voice_name,
        websocket_url = %session.websocket_url,
        "create realtime session completed"
    );
    Ok(session)
}

#[tauri::command]
pub async fn start_realtime_session(state: State<'_, AppState>, session_id: String) -> ApiResult<RealtimeSession> {
    tracing::debug!(%session_id, "start realtime session requested");
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    let format = PcmFormat {
        sample_rate: settings.runtime.default_sample_rate,
        frame_ms: settings.runtime.audio_frame_ms,
        sample_format: SampleFormat::I16,
        ..PcmFormat::default()
    };
    tracing::debug!(
        %session_id,
        sample_rate = format.sample_rate,
        frame_ms = format.frame_ms,
        virtual_mic_enabled = settings.device.virtual_mic_enabled,
        input_device_id = ?settings.device.input_device_id,
        virtual_mic_device_id = ?settings.device.virtual_mic_device_id,
        realtime_voice_mode = ?settings.runtime.realtime_voice_mode,
        "realtime audio settings resolved"
    );
    if settings.device.virtual_mic_enabled {
        state
            .virtual_mic()
            .set_target_device_id(settings.device.virtual_mic_device_id.clone());
    }

    let started = state
        .sessions()
        .start_realtime_session(&session_id, state.audio_engine())
        .map_err(ApiError::from);
    if started.is_err() && settings.device.virtual_mic_enabled {
        tracing::warn!(%session_id, "session start failed before stream start; stopping virtual mic");
        let _ = state.virtual_mic().stop();
    }
    let session = started?;
    if let Err(error) = state
        .realtime_streams()
        .start(
            session.clone(),
            format,
            None,
            state.virtual_mic_handle(),
            settings.device.virtual_mic_enabled,
            RealtimeStreamMode::RealtimeVoice,
        )
        .await
    {
        tracing::warn!(%session_id, %error, "failed to start realtime stream");
        let _ = state
            .sessions()
            .mark_realtime_session_failed(&session_id, error.to_string(), state.audio_engine());
        let _ = state.virtual_mic().stop();
        return Err(ApiError::from(error));
    }
    tracing::debug!(
        %session_id,
        trace_id = %session.trace_id,
        voice_name = %session.voice_name,
        "start realtime session completed"
    );
    Ok(session)
}

#[tauri::command]
pub fn start_realtime_input(state: State<'_, AppState>, session_id: String) -> ApiResult<RealtimeStreamSnapshot> {
    tracing::debug!(%session_id, "start realtime input requested");
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
    let input_device = state
        .audio_devices()
        .input_device_by_id(settings.device.input_device_id.as_deref())
        .map_err(ApiError::from)?;
    if let Err(error) = state.realtime_streams().start_input(&session_id, input_device) {
        let _ = state.virtual_mic().stop();
        return Err(ApiError::from(error));
    }
    state
        .realtime_streams()
        .get_snapshot(&session_id)
        .map_err(ApiError::from)
}

#[tauri::command]
pub fn stop_realtime_input(state: State<'_, AppState>, session_id: String) -> ApiResult<RealtimeStreamSnapshot> {
    tracing::debug!(%session_id, "stop realtime input requested");
    state
        .realtime_streams()
        .stop_input(&session_id)
        .map_err(ApiError::from)?;
    let _ = state.virtual_mic().stop();
    state
        .realtime_streams()
        .get_snapshot(&session_id)
        .map_err(ApiError::from)
}

#[tauri::command]
pub fn start_realtime_monitor(state: State<'_, AppState>, session_id: String) -> ApiResult<RealtimeStreamSnapshot> {
    tracing::debug!(%session_id, "start realtime monitor requested");
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    let output_device = state
        .audio_devices()
        .output_device_by_id(settings.device.output_device_id.as_deref())
        .map_err(ApiError::from)?;
    state
        .realtime_streams()
        .start_monitor(&session_id, output_device)
        .map_err(ApiError::from)?;
    state
        .realtime_streams()
        .get_snapshot(&session_id)
        .map_err(ApiError::from)
}

#[tauri::command]
pub fn stop_realtime_monitor(state: State<'_, AppState>, session_id: String) -> ApiResult<RealtimeStreamSnapshot> {
    tracing::debug!(%session_id, "stop realtime monitor requested");
    state
        .realtime_streams()
        .stop_monitor(&session_id)
        .map_err(ApiError::from)?;
    state
        .realtime_streams()
        .get_snapshot(&session_id)
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn stop_realtime_session(state: State<'_, AppState>, session_id: String) -> ApiResult<RealtimeSession> {
    tracing::debug!(%session_id, "stop realtime session requested");
    let _ = state.realtime_streams().stop(&session_id).await;
    let stopped = state
        .sessions()
        .stop_realtime_session(&session_id, state.audio_engine())
        .map_err(ApiError::from);
    let _ = state.virtual_mic().stop();
    tracing::debug!(%session_id, "stop realtime session completed");
    stopped
}

#[tauri::command]
pub async fn fail_realtime_session(
    state: State<'_, AppState>,
    session_id: String,
    message: String,
) -> ApiResult<RealtimeSession> {
    tracing::debug!(%session_id, %message, "fail realtime session requested");
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
    tracing::debug!(%session_id, params = ?request.runtime_params.values, "update realtime params requested");
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
    tracing::debug!(%session_id, voice_name = %request.voice_name, "switch realtime voice requested");
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
    let snapshot = state
        .realtime_streams()
        .get_snapshot(&session_id)
        .map_err(ApiError::from)?;
    tracing::debug!(
        %session_id,
        websocket_state = %snapshot.websocket_state,
        sent_frames = snapshot.sent_frames,
        received_frames = snapshot.received_frames,
        latency_ms = ?snapshot.latency_ms,
        last_event = ?snapshot.last_event,
        last_error = ?snapshot.last_error,
        "realtime stream snapshot requested"
    );
    Ok(snapshot)
}

#[tauri::command]
pub fn list_realtime_stream_snapshots(state: State<'_, AppState>) -> Vec<RealtimeStreamSnapshot> {
    state.realtime_streams().list_snapshots()
}
