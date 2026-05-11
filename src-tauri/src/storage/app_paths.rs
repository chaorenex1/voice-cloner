use std::{env, path::PathBuf};

use serde::Serialize;

use crate::app::error::{AppError, AppResult};

const APP_DIR_NAME: &str = "voice-cloner";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    root: PathBuf,
    settings_dir: PathBuf,
    cache_dir: PathBuf,
    preset_preview_dir: PathBuf,
    offline_inputs_dir: PathBuf,
    voice_design_artifacts_dir: PathBuf,
    voice_separation_jobs_dir: PathBuf,
    offline_exports_dir: PathBuf,
    library_dir: PathBuf,
    custom_voices_dir: PathBuf,
    offline_jobs_file: PathBuf,
    sync_state_file: PathBuf,
}

impl AppPaths {
    pub fn discover() -> AppResult<Self> {
        if let Some(root) = env::var_os("VOICE_CLONER_HOME") {
            return Self::from_root(PathBuf::from(root));
        }

        let home = env::var_os("USERPROFILE")
            .or_else(|| env::var_os("HOME"))
            .ok_or_else(|| AppError::invalid_settings("USERPROFILE/HOME is not available"))?;

        Self::from_root(PathBuf::from(home).join(APP_DIR_NAME))
    }

    pub fn from_root(root: impl Into<PathBuf>) -> AppResult<Self> {
        let root = root.into();
        let settings_dir = root.join("settings");
        let cache_dir = root.join("cache");
        let preset_preview_dir = cache_dir.join("preset-preview");
        let offline_inputs_dir = cache_dir.join("offline-inputs");
        let voice_design_artifacts_dir = cache_dir.join("voice-design-artifacts");
        let voice_separation_jobs_dir = cache_dir.join("voice-separation-jobs");
        let offline_exports_dir = cache_dir.join("offline-exports");
        let library_dir = root.join("library");
        let custom_voices_dir = library_dir.join("custom-voices");
        let offline_jobs_file = library_dir.join("offline-jobs.json");
        let sync_state_file = library_dir.join("sync-state.json");

        let paths = Self {
            root,
            settings_dir,
            cache_dir,
            preset_preview_dir,
            offline_inputs_dir,
            voice_design_artifacts_dir,
            voice_separation_jobs_dir,
            offline_exports_dir,
            library_dir,
            custom_voices_dir,
            offline_jobs_file,
            sync_state_file,
        };
        paths.ensure()?;
        Ok(paths)
    }

    pub fn ensure(&self) -> AppResult<()> {
        for path in [
            &self.root,
            &self.settings_dir,
            &self.preset_preview_dir,
            &self.offline_inputs_dir,
            &self.voice_design_artifacts_dir,
            &self.voice_separation_jobs_dir,
            &self.offline_exports_dir,
            &self.library_dir,
            &self.custom_voices_dir,
        ] {
            std::fs::create_dir_all(path).map_err(|source| AppError::io("creating app directory", source))?;
        }
        Ok(())
    }

    pub fn root(&self) -> PathBuf {
        self.root.clone()
    }

    pub fn settings_file(&self) -> PathBuf {
        self.settings_dir.join("app-settings.json")
    }

    pub fn preset_preview_dir(&self) -> PathBuf {
        self.preset_preview_dir.clone()
    }

    pub fn offline_inputs_dir(&self) -> PathBuf {
        self.offline_inputs_dir.clone()
    }

    pub fn voice_design_artifacts_dir(&self) -> PathBuf {
        self.voice_design_artifacts_dir.clone()
    }

    pub fn voice_separation_jobs_dir(&self) -> PathBuf {
        self.voice_separation_jobs_dir.clone()
    }

    pub fn offline_exports_dir(&self) -> PathBuf {
        self.offline_exports_dir.clone()
    }

    pub fn custom_voices_dir(&self) -> PathBuf {
        self.custom_voices_dir.clone()
    }

    pub fn offline_jobs_file(&self) -> PathBuf {
        self.offline_jobs_file.clone()
    }

    pub fn sync_state_file(&self) -> PathBuf {
        self.sync_state_file.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::AppPaths;

    #[test]
    fn app_paths_match_architecture_layout() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("voice-cloner-paths-{unique}"));
        let paths = AppPaths::from_root(&root).unwrap();

        assert_eq!(paths.settings_file(), root.join("settings/app-settings.json"));
        assert_eq!(paths.preset_preview_dir(), root.join("cache/preset-preview"));
        assert_eq!(paths.offline_inputs_dir(), root.join("cache/offline-inputs"));
        assert_eq!(
            paths.voice_design_artifacts_dir(),
            root.join("cache/voice-design-artifacts")
        );
        assert_eq!(
            paths.voice_separation_jobs_dir(),
            root.join("cache/voice-separation-jobs")
        );
        assert_eq!(paths.offline_exports_dir(), root.join("cache/offline-exports"));
        assert_eq!(paths.custom_voices_dir(), root.join("library/custom-voices"));
        assert_eq!(paths.offline_jobs_file(), root.join("library/offline-jobs.json"));
        assert_eq!(paths.sync_state_file(), root.join("library/sync-state.json"));
        assert!(paths.settings_file().parent().unwrap().exists());
        assert!(paths.custom_voices_dir().exists());
        assert!(paths.sync_state_file().parent().unwrap().exists());
    }
}
