use std::{collections::BTreeSet, sync::RwLock};

use serde::Serialize;

use crate::{
    app::error::{AppError, AppResult},
    audio::frame::PcmFormat,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AudioEngineSnapshot {
    pub active_session_ids: Vec<String>,
    pub format: PcmFormat,
}

#[derive(Debug)]
pub struct AudioEngine {
    active_session_ids: RwLock<BTreeSet<String>>,
    format: PcmFormat,
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new(PcmFormat::default())
    }
}

impl AudioEngine {
    pub fn new(format: PcmFormat) -> Self {
        Self {
            active_session_ids: RwLock::new(BTreeSet::new()),
            format,
        }
    }

    pub fn start_realtime_session(&self, session_id: &str) -> AppResult<AudioEngineSnapshot> {
        self.format.validate().map_err(AppError::audio)?;
        self.active_session_ids
            .write()
            .expect("audio engine lock poisoned")
            .insert(session_id.to_string());
        Ok(self.snapshot())
    }

    pub fn stop_realtime_session(&self, session_id: &str) -> AudioEngineSnapshot {
        self.active_session_ids
            .write()
            .expect("audio engine lock poisoned")
            .remove(session_id);
        self.snapshot()
    }

    pub fn stop_all(&self) -> AudioEngineSnapshot {
        self.active_session_ids
            .write()
            .expect("audio engine lock poisoned")
            .clear();
        self.snapshot()
    }

    pub fn snapshot(&self) -> AudioEngineSnapshot {
        AudioEngineSnapshot {
            active_session_ids: self
                .active_session_ids
                .read()
                .expect("audio engine lock poisoned")
                .iter()
                .cloned()
                .collect(),
            format: self.format,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AudioEngine;

    #[test]
    fn audio_engine_tracks_active_realtime_sessions() {
        let engine = AudioEngine::default();

        let started = engine.start_realtime_session("session-1").unwrap();
        assert_eq!(started.active_session_ids, vec!["session-1"]);

        let stopped = engine.stop_realtime_session("session-1");
        assert!(stopped.active_session_ids.is_empty());
    }
}
