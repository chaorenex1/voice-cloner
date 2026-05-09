use std::sync::Arc;

use crate::{
    audio::{
        device_manager::AudioDeviceManager, engine::AudioEngine, preview_player::VoicePreviewPlayer,
        virtual_mic::SelectableVirtualMicAdapter,
    },
    services::{
        asset_cache::AssetCache, offline_job_manager::OfflineJobManager,
        realtime_stream_manager::RealtimeStreamManager, session_manager::SessionManager,
        settings_manager::SettingsManager, voice_design_manager::VoiceDesignManager, voice_library::VoiceLibrary,
        voice_sync_manager::VoiceSyncManager,
    },
    storage::app_paths::AppPaths,
};

#[derive(Clone)]
pub struct AppState {
    paths: AppPaths,
    settings: Arc<SettingsManager>,
    audio_devices: Arc<AudioDeviceManager>,
    audio_engine: Arc<AudioEngine>,
    voice_preview: Arc<VoicePreviewPlayer>,
    virtual_mic: Arc<SelectableVirtualMicAdapter>,
    realtime_streams: Arc<RealtimeStreamManager>,
    sessions: Arc<SessionManager>,
    offline_jobs: Arc<OfflineJobManager>,
    asset_cache: Arc<AssetCache>,
    voice_design: Arc<VoiceDesignManager>,
    voice_library: Arc<VoiceLibrary>,
    voice_sync: Arc<VoiceSyncManager>,
}

impl AppState {
    pub fn new(paths: AppPaths) -> crate::app::error::AppResult<Self> {
        let settings = SettingsManager::new(paths.settings_file());
        let asset_cache = AssetCache::new_with_inputs(
            paths.offline_exports_dir(),
            paths.offline_inputs_dir(),
            paths.voice_design_artifacts_dir(),
        )?;
        let voice_library = VoiceLibrary::new(paths.custom_voices_dir())?;
        let voice_sync = VoiceSyncManager::new(paths.sync_state_file());

        let loaded_settings = settings.load_or_default()?;
        let virtual_mic = SelectableVirtualMicAdapter::default();
        virtual_mic.set_target_device_id(loaded_settings.device.virtual_mic_device_id.clone());
        voice_sync.load_or_default()?;
        let offline_jobs = OfflineJobManager::new(paths.offline_jobs_file())?;

        Ok(Self {
            paths,
            settings: Arc::new(settings),
            audio_devices: Arc::new(AudioDeviceManager::default()),
            audio_engine: Arc::new(AudioEngine::default()),
            voice_preview: Arc::new(VoicePreviewPlayer::default()),
            virtual_mic: Arc::new(virtual_mic),
            realtime_streams: Arc::new(RealtimeStreamManager::default()),
            sessions: Arc::new(SessionManager::default()),
            offline_jobs: Arc::new(offline_jobs),
            asset_cache: Arc::new(asset_cache),
            voice_design: Arc::new(VoiceDesignManager::default()),
            voice_library: Arc::new(voice_library),
            voice_sync: Arc::new(voice_sync),
        })
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn settings(&self) -> &SettingsManager {
        &self.settings
    }

    pub fn audio_devices(&self) -> &AudioDeviceManager {
        &self.audio_devices
    }

    pub fn audio_engine(&self) -> &AudioEngine {
        &self.audio_engine
    }

    pub fn voice_preview(&self) -> &VoicePreviewPlayer {
        &self.voice_preview
    }

    pub fn virtual_mic(&self) -> &SelectableVirtualMicAdapter {
        &self.virtual_mic
    }

    pub fn virtual_mic_handle(&self) -> Arc<SelectableVirtualMicAdapter> {
        Arc::clone(&self.virtual_mic)
    }

    pub fn realtime_streams(&self) -> &RealtimeStreamManager {
        &self.realtime_streams
    }

    pub fn sessions(&self) -> &SessionManager {
        &self.sessions
    }

    pub fn offline_jobs(&self) -> &OfflineJobManager {
        &self.offline_jobs
    }

    pub fn asset_cache(&self) -> &AssetCache {
        &self.asset_cache
    }

    pub fn voice_design(&self) -> &VoiceDesignManager {
        &self.voice_design
    }

    pub fn voice_library(&self) -> &VoiceLibrary {
        &self.voice_library
    }

    pub fn voice_sync(&self) -> &VoiceSyncManager {
        &self.voice_sync
    }
}
