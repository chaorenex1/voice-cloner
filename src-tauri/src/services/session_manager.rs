use std::{collections::BTreeMap, sync::RwLock};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    app::{
        error::{AppError, AppResult},
        trace::{new_entity_id, TraceId},
    },
    audio::engine::AudioEngine,
    clients::funspeech::realtime::RealtimeEndpoint,
    domain::{
        runtime_params::RuntimeParams,
        session::{RealtimeSession, RealtimeSessionStatus},
        settings::AppSettings,
    },
};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateRealtimeSessionRequest {
    pub voice_name: String,
    #[serde(default)]
    pub runtime_params: RuntimeParams,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRealtimeParamsRequest {
    #[serde(default)]
    pub runtime_params: RuntimeParams,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SwitchRealtimeVoiceRequest {
    pub voice_name: String,
}

#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: RwLock<BTreeMap<String, RealtimeSession>>,
}

impl SessionManager {
    pub fn create_realtime_session(
        &self,
        request: CreateRealtimeSessionRequest,
        settings: &AppSettings,
    ) -> AppResult<RealtimeSession> {
        let voice_name = request.voice_name.trim();
        if voice_name.is_empty() {
            return Err(AppError::realtime_session("voiceName is required"));
        }
        settings.validate().map_err(AppError::invalid_settings)?;
        let endpoint = RealtimeEndpoint::from_backend_config(&settings.backend.realtime);
        let now = Utc::now();
        let session = RealtimeSession {
            session_id: new_entity_id("session"),
            trace_id: TraceId::new("realtime").into_string(),
            voice_name: voice_name.to_string(),
            runtime_params: request.runtime_params,
            status: RealtimeSessionStatus::Idle,
            websocket_url: endpoint.websocket_url,
            error_summary: None,
            created_at: now,
            updated_at: now,
        };

        self.sessions
            .write()
            .expect("session manager lock poisoned")
            .insert(session.session_id.clone(), session.clone());
        Ok(session)
    }

    pub fn start_realtime_session(&self, session_id: &str, audio_engine: &AudioEngine) -> AppResult<RealtimeSession> {
        let mut sessions = self.sessions.write().expect("session manager lock poisoned");
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::realtime_session(format!("session not found: {session_id}")))?;

        match session.status {
            RealtimeSessionStatus::Idle | RealtimeSessionStatus::Stopped | RealtimeSessionStatus::Failed => {}
            _ => {
                return Err(AppError::realtime_session(format!(
                    "cannot start realtime session from {:?}",
                    session.status
                )));
            }
        }

        session.error_summary = None;
        session.transition_to(RealtimeSessionStatus::Connecting);
        audio_engine.start_realtime_session(session_id)?;
        session.transition_to(RealtimeSessionStatus::Running);
        Ok(session.clone())
    }

    pub fn stop_realtime_session(&self, session_id: &str, audio_engine: &AudioEngine) -> AppResult<RealtimeSession> {
        let mut sessions = self.sessions.write().expect("session manager lock poisoned");
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::realtime_session(format!("session not found: {session_id}")))?;

        session.transition_to(RealtimeSessionStatus::Stopping);
        audio_engine.stop_realtime_session(session_id);
        session.transition_to(RealtimeSessionStatus::Stopped);
        Ok(session.clone())
    }

    pub fn mark_realtime_session_failed(
        &self,
        session_id: &str,
        message: impl Into<String>,
        audio_engine: &AudioEngine,
    ) -> AppResult<RealtimeSession> {
        let mut sessions = self.sessions.write().expect("session manager lock poisoned");
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::realtime_session(format!("session not found: {session_id}")))?;

        audio_engine.stop_realtime_session(session_id);
        session.error_summary = Some(message.into());
        session.transition_to(RealtimeSessionStatus::Failed);
        Ok(session.clone())
    }

    pub fn update_realtime_params(
        &self,
        session_id: &str,
        request: UpdateRealtimeParamsRequest,
    ) -> AppResult<RealtimeSession> {
        let mut sessions = self.sessions.write().expect("session manager lock poisoned");
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::realtime_session(format!("session not found: {session_id}")))?;

        session.runtime_params = request.runtime_params;
        session.updated_at = Utc::now();
        Ok(session.clone())
    }

    pub fn switch_realtime_voice(
        &self,
        session_id: &str,
        request: SwitchRealtimeVoiceRequest,
    ) -> AppResult<RealtimeSession> {
        let voice_name = request.voice_name.trim();
        if voice_name.is_empty() {
            return Err(AppError::realtime_session("voiceName is required"));
        }

        let mut sessions = self.sessions.write().expect("session manager lock poisoned");
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::realtime_session(format!("session not found: {session_id}")))?;

        session.voice_name = voice_name.to_string();
        session.updated_at = Utc::now();
        Ok(session.clone())
    }

    pub fn get_realtime_session(&self, session_id: &str) -> AppResult<RealtimeSession> {
        self.sessions
            .read()
            .expect("session manager lock poisoned")
            .get(session_id)
            .cloned()
            .ok_or_else(|| AppError::realtime_session(format!("session not found: {session_id}")))
    }

    pub fn list_realtime_sessions(&self) -> Vec<RealtimeSession> {
        self.sessions
            .read()
            .expect("session manager lock poisoned")
            .values()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        audio::engine::AudioEngine,
        domain::{runtime_params::RuntimeParams, session::RealtimeSessionStatus, settings::AppSettings},
    };

    use super::{
        CreateRealtimeSessionRequest, SessionManager, SwitchRealtimeVoiceRequest, UpdateRealtimeParamsRequest,
    };

    #[test]
    fn session_manager_creates_starts_updates_switches_and_stops_session() {
        let manager = SessionManager::default();
        let audio = AudioEngine::default();
        let settings = AppSettings::default();
        let created = manager
            .create_realtime_session(
                CreateRealtimeSessionRequest {
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                },
                &settings,
            )
            .unwrap();

        assert_eq!(created.status, RealtimeSessionStatus::Idle);
        assert!(created.websocket_url.ends_with("/ws/v1/realtime/voice"));

        let running = manager.start_realtime_session(&created.session_id, &audio).unwrap();
        assert_eq!(running.status, RealtimeSessionStatus::Running);
        assert_eq!(audio.snapshot().active_session_ids, vec![created.session_id.clone()]);

        let mut params = RuntimeParams::default();
        params.values.insert("pitch".into(), json!(1.2));
        let updated = manager
            .update_realtime_params(
                &created.session_id,
                UpdateRealtimeParamsRequest { runtime_params: params },
            )
            .unwrap();
        assert_eq!(updated.runtime_params.values.get("pitch"), Some(&json!(1.2)));

        let switched = manager
            .switch_realtime_voice(
                &created.session_id,
                SwitchRealtimeVoiceRequest {
                    voice_name: "robot".into(),
                },
            )
            .unwrap();
        assert_eq!(switched.voice_name, "robot");

        let stopped = manager.stop_realtime_session(&created.session_id, &audio).unwrap();
        assert_eq!(stopped.status, RealtimeSessionStatus::Stopped);
        assert!(audio.snapshot().active_session_ids.is_empty());
    }

    #[test]
    fn session_manager_marks_failed_sessions_and_clears_audio_state() {
        let manager = SessionManager::default();
        let audio = AudioEngine::default();
        let settings = AppSettings::default();
        let created = manager
            .create_realtime_session(
                CreateRealtimeSessionRequest {
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                },
                &settings,
            )
            .unwrap();
        manager.start_realtime_session(&created.session_id, &audio).unwrap();

        let failed = manager
            .mark_realtime_session_failed(&created.session_id, "websocket disconnected", &audio)
            .unwrap();

        assert_eq!(failed.status, RealtimeSessionStatus::Failed);
        assert_eq!(failed.error_summary.as_deref(), Some("websocket disconnected"));
        assert!(audio.snapshot().active_session_ids.is_empty());
    }

    #[test]
    fn session_manager_rejects_empty_voice_name() {
        let error = SessionManager::default()
            .create_realtime_session(
                CreateRealtimeSessionRequest {
                    voice_name: " ".into(),
                    runtime_params: RuntimeParams::default(),
                },
                &AppSettings::default(),
            )
            .unwrap_err();

        assert!(error.to_string().contains("voiceName"));
    }
}
