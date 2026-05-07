use std::path::PathBuf;

use chrono::Utc;

use crate::{
    app::{
        error::{AppError, AppResult},
        trace::TraceId,
    },
    domain::{
        settings::AppSettings,
        voice::SyncStatus,
        voice_sync::{VoiceSyncEndpointSet, VoiceSyncOperation, VoiceSyncReport, VoiceSyncState},
    },
    services::voice_library::VoiceLibrary,
    storage::json_store::JsonStore,
};

#[derive(Debug)]
pub struct VoiceSyncManager {
    store: JsonStore<VoiceSyncState>,
}

impl VoiceSyncManager {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            store: JsonStore::new(path, VoiceSyncState::default()),
        }
    }

    pub fn load_or_default(&self) -> AppResult<VoiceSyncState> {
        self.store.load_or_create()
    }

    pub fn full_sync(&self, library: &VoiceLibrary, settings: &AppSettings) -> AppResult<VoiceSyncReport> {
        settings.validate().map_err(AppError::invalid_settings)?;
        let endpoints = VoiceSyncEndpointSet::from_backend_config(&settings.backend.tts);
        let local_voice_count = library.list_custom_voices()?.len();
        self.push_report(VoiceSyncReport {
            operation: VoiceSyncOperation::FullSync,
            trace_id: TraceId::new("voice-sync").into_string(),
            endpoint_url: endpoints.sync_url,
            voice_name: None,
            local_voice_count,
            sync_status: None,
            message: "full sync endpoint prepared; real FunSpeech pull is not connected yet".into(),
            created_at: Utc::now(),
        })
    }

    pub fn register_voice(
        &self,
        voice_name: &str,
        library: &VoiceLibrary,
        settings: &AppSettings,
    ) -> AppResult<VoiceSyncReport> {
        self.incremental_voice_sync(
            VoiceSyncOperation::Register,
            voice_name,
            SyncStatus::Synced,
            library,
            settings,
        )
    }

    pub fn update_voice(
        &self,
        voice_name: &str,
        library: &VoiceLibrary,
        settings: &AppSettings,
    ) -> AppResult<VoiceSyncReport> {
        self.incremental_voice_sync(
            VoiceSyncOperation::Update,
            voice_name,
            SyncStatus::Synced,
            library,
            settings,
        )
    }

    pub fn mark_voice_sync_failed(
        &self,
        operation: VoiceSyncOperation,
        voice_name: &str,
        message: impl Into<String>,
        library: &VoiceLibrary,
        settings: &AppSettings,
    ) -> AppResult<VoiceSyncReport> {
        settings.validate().map_err(AppError::invalid_settings)?;
        let endpoints = VoiceSyncEndpointSet::from_backend_config(&settings.backend.tts);
        let profile = library.mark_sync_status(voice_name, SyncStatus::Failed)?;
        self.push_report(VoiceSyncReport {
            endpoint_url: endpoint_for_operation(&endpoints, &operation),
            operation,
            trace_id: TraceId::new("voice-sync").into_string(),
            voice_name: Some(profile.voice_name),
            local_voice_count: library.list_custom_voices()?.len(),
            sync_status: Some(SyncStatus::Failed),
            message: message.into(),
            created_at: Utc::now(),
        })
    }

    pub fn delete_voice(
        &self,
        voice_name: &str,
        library: &VoiceLibrary,
        settings: &AppSettings,
    ) -> AppResult<VoiceSyncReport> {
        settings.validate().map_err(AppError::invalid_settings)?;
        let endpoints = VoiceSyncEndpointSet::from_backend_config(&settings.backend.tts);
        let deleted = library.delete_custom_voice(voice_name)?;
        self.push_report(VoiceSyncReport {
            operation: VoiceSyncOperation::Delete,
            trace_id: TraceId::new("voice-sync").into_string(),
            endpoint_url: endpoints.delete_url,
            voice_name: Some(deleted.voice_name),
            local_voice_count: library.list_custom_voices()?.len(),
            sync_status: Some(SyncStatus::Synced),
            message: "delete sync marked complete locally; real FunSpeech request is not connected yet".into(),
            created_at: Utc::now(),
        })
    }

    pub fn refresh_runtime(&self, library: &VoiceLibrary, settings: &AppSettings) -> AppResult<VoiceSyncReport> {
        settings.validate().map_err(AppError::invalid_settings)?;
        let endpoints = VoiceSyncEndpointSet::from_backend_config(&settings.backend.tts);
        self.push_report(VoiceSyncReport {
            operation: VoiceSyncOperation::Refresh,
            trace_id: TraceId::new("voice-sync").into_string(),
            endpoint_url: endpoints.refresh_url,
            voice_name: None,
            local_voice_count: library.list_custom_voices()?.len(),
            sync_status: None,
            message: "voice runtime refresh endpoint prepared; real FunSpeech request is not connected yet".into(),
            created_at: Utc::now(),
        })
    }

    pub fn list_reports(&self) -> Vec<VoiceSyncReport> {
        self.store.get().reports
    }

    fn incremental_voice_sync(
        &self,
        operation: VoiceSyncOperation,
        voice_name: &str,
        target_status: SyncStatus,
        library: &VoiceLibrary,
        settings: &AppSettings,
    ) -> AppResult<VoiceSyncReport> {
        settings.validate().map_err(AppError::invalid_settings)?;
        let endpoints = VoiceSyncEndpointSet::from_backend_config(&settings.backend.tts);
        let profile = library.mark_sync_status(voice_name, target_status.clone())?;
        self.push_report(VoiceSyncReport {
            endpoint_url: endpoint_for_operation(&endpoints, &operation),
            operation,
            trace_id: TraceId::new("voice-sync").into_string(),
            voice_name: Some(profile.voice_name),
            local_voice_count: library.list_custom_voices()?.len(),
            sync_status: Some(target_status),
            message: "incremental voice sync marked complete locally; real FunSpeech request is not connected yet"
                .into(),
            created_at: Utc::now(),
        })
    }

    fn push_report(&self, report: VoiceSyncReport) -> AppResult<VoiceSyncReport> {
        let mut state = self.store.get();
        state.reports.push(report.clone());
        self.store.replace(state)?;
        Ok(report)
    }
}

fn endpoint_for_operation(endpoints: &VoiceSyncEndpointSet, operation: &VoiceSyncOperation) -> String {
    match operation {
        VoiceSyncOperation::FullSync => endpoints.sync_url.clone(),
        VoiceSyncOperation::Register => endpoints.register_url.clone(),
        VoiceSyncOperation::Update => endpoints.update_url.clone(),
        VoiceSyncOperation::Delete => endpoints.delete_url.clone(),
        VoiceSyncOperation::Refresh => endpoints.refresh_url.clone(),
    }
}

pub fn parse_incremental_operation(value: &str) -> AppResult<VoiceSyncOperation> {
    match value {
        "register" => Ok(VoiceSyncOperation::Register),
        "update" => Ok(VoiceSyncOperation::Update),
        "delete" => Ok(VoiceSyncOperation::Delete),
        other => Err(AppError::offline_job(format!(
            "unsupported voice sync operation for failure report: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use chrono::Utc;

    use crate::{
        domain::{
            settings::AppSettings,
            voice::{CustomVoiceProfile, SyncStatus},
            voice_sync::VoiceSyncOperation,
        },
        services::voice_library::VoiceLibrary,
    };

    use super::VoiceSyncManager;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("voice-cloner-{name}-{unique}"))
    }

    fn library() -> VoiceLibrary {
        VoiceLibrary::new(temp_path("sync-library")).unwrap()
    }

    fn manager() -> VoiceSyncManager {
        VoiceSyncManager::new(temp_path("sync-state").join("sync-state.json"))
    }

    fn reference_audio_path() -> String {
        let source_dir = temp_path("sync-source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("preview.wav");
        std::fs::write(&path, b"sync preview wav").unwrap();
        path.to_string_lossy().into_owned()
    }

    fn seed_voice(library: &VoiceLibrary) {
        library
            .save_custom_voice(CustomVoiceProfile {
                voice_name: "sync-me".into(),
                source_prompt_text: Some("warm".into()),
                asr_text: None,
                voice_instruction: "warm".into(),
                reference_audio_path: reference_audio_path(),
                reference_text: "hello".into(),
                sync_status: SyncStatus::PendingSync,
                last_synced_at: None,
                created_at: Utc::now(),
            })
            .unwrap();
    }

    #[test]
    fn voice_sync_manager_marks_register_update_and_refresh_reports() {
        let library = library();
        seed_voice(&library);
        let manager = manager();
        let settings = AppSettings::default();

        let full = manager.full_sync(&library, &settings).unwrap();
        assert_eq!(full.operation, VoiceSyncOperation::FullSync);
        assert!(full.endpoint_url.ends_with("/voices/v1/sync"));

        let registered = manager.register_voice("sync-me", &library, &settings).unwrap();
        assert_eq!(registered.sync_status, Some(SyncStatus::Synced));
        assert!(registered.endpoint_url.ends_with("/voices/v1/register"));
        assert_eq!(
            library.get_custom_voice("sync-me").unwrap().sync_status,
            SyncStatus::Synced
        );

        let updated = manager.update_voice("sync-me", &library, &settings).unwrap();
        assert!(updated.endpoint_url.ends_with("/voices/v1/update"));

        let refreshed = manager.refresh_runtime(&library, &settings).unwrap();
        assert!(refreshed.endpoint_url.ends_with("/voices/v1/refresh"));
        assert_eq!(manager.list_reports().len(), 4);
    }

    #[test]
    fn voice_sync_manager_persists_reports_to_sync_state_file() {
        let library = library();
        seed_voice(&library);
        let path = temp_path("sync-state-persist").join("sync-state.json");
        let manager = VoiceSyncManager::new(&path);

        manager.full_sync(&library, &AppSettings::default()).unwrap();

        let reloaded = VoiceSyncManager::new(&path);
        assert_eq!(reloaded.load_or_default().unwrap().reports.len(), 1);
        assert_eq!(reloaded.list_reports().len(), 1);
    }

    #[test]
    fn voice_sync_manager_marks_failure_and_delete() {
        let library = library();
        seed_voice(&library);
        let manager = manager();
        let settings = AppSettings::default();

        let failed = manager
            .mark_voice_sync_failed(
                VoiceSyncOperation::Register,
                "sync-me",
                "backend unavailable",
                &library,
                &settings,
            )
            .unwrap();
        assert_eq!(failed.sync_status, Some(SyncStatus::Failed));
        assert_eq!(
            library.get_custom_voice("sync-me").unwrap().sync_status,
            SyncStatus::Failed
        );

        let deleted = manager.delete_voice("sync-me", &library, &settings).unwrap();
        assert_eq!(deleted.operation, VoiceSyncOperation::Delete);
        assert!(library.list_custom_voices().unwrap().is_empty());
    }
}
