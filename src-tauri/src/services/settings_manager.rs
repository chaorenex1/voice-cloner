use std::path::PathBuf;

use crate::{
    app::error::{AppError, AppResult},
    domain::settings::{AppSettings, AppSettingsPatch},
    storage::json_store::JsonStore,
};

#[derive(Debug)]
pub struct SettingsManager {
    store: JsonStore<AppSettings>,
}

impl SettingsManager {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            store: JsonStore::new(path, AppSettings::default()),
        }
    }

    pub fn load_or_default(&self) -> AppResult<AppSettings> {
        let loaded = self.store.load_or_create()?;
        let mut settings = loaded.clone();
        settings.normalize_for_local_save();
        settings.validate().map_err(AppError::invalid_settings)?;
        if settings != loaded {
            self.store.replace(settings.clone())?;
        }
        Ok(settings)
    }

    pub fn get(&self) -> AppSettings {
        self.store.get()
    }

    pub fn update(&self, patch: AppSettingsPatch) -> AppResult<AppSettings> {
        let mut next = patch.apply_to(self.get());
        next.normalize_for_local_save();
        next.validate().map_err(AppError::invalid_settings)?;
        self.store.replace(next)
    }

    pub fn reset(&self) -> AppResult<AppSettings> {
        self.store.replace(AppSettings::default())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::domain::settings::{
        AppSettings, AppSettingsPatch, BackendSettingsPatch, DeviceSettingsPatch, RuntimeSettingsPatch,
    };

    use super::SettingsManager;

    fn test_settings_path() -> std::path::PathBuf {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("voice-cloner-settings-{unique}/settings.json"))
    }

    #[test]
    fn settings_manager_creates_updates_and_resets_settings() {
        let manager = SettingsManager::new(test_settings_path());

        let defaults = manager.load_or_default().unwrap();
        assert_eq!(defaults.backend.realtime.provider_name, "funspeech");

        let updated = manager
            .update(AppSettingsPatch {
                device: Some(DeviceSettingsPatch {
                    input_device_id: Some(Some("mic-1".into())),
                    ..Default::default()
                }),
                runtime: Some(RuntimeSettingsPatch {
                    default_output_format: Some("mp3".into()),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(updated.device.input_device_id.as_deref(), Some("mic-1"));
        assert_eq!(updated.runtime.default_output_format, "mp3");

        let reset = manager.reset().unwrap();
        assert_eq!(reset.device.input_device_id, None);
        assert_eq!(reset.runtime.default_output_format, "wav");
    }

    #[test]
    fn settings_manager_normalizes_hidden_provider_fields_before_saving() {
        let manager = SettingsManager::new(test_settings_path());
        manager.load_or_default().unwrap();
        let mut llm = AppSettings::default().backend.llm;
        llm.provider_name = " ".into();
        let mut realtime = AppSettings::default().backend.realtime;
        realtime.provider_name = "custom-provider-from-old-ui".into();
        realtime.model = Some("stale-funspeech-model".into());

        let saved = manager
            .update(AppSettingsPatch {
                backend: Some(BackendSettingsPatch {
                    llm: Some(llm),
                    realtime: Some(realtime),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(saved.backend.llm.provider_name, "local-llm");
        assert_eq!(saved.backend.realtime.provider_name, "funspeech");
        assert_eq!(saved.backend.realtime.model, None);
    }
}
