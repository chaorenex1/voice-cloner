use std::{path::PathBuf, sync::RwLock};

use chrono::Utc;

use crate::{
    app::error::{AppError, AppResult},
    domain::{
        voice::{CustomVoiceProfile, SyncStatus},
        voice_sync::RemoteVoiceInfo,
    },
    storage::json_store::JsonStore,
};

#[derive(Debug)]
pub struct VoiceLibrary {
    custom_voices_dir: PathBuf,
    names: RwLock<Vec<String>>,
}

impl VoiceLibrary {
    pub fn new(custom_voices_dir: impl Into<PathBuf>) -> AppResult<Self> {
        let custom_voices_dir = custom_voices_dir.into();
        std::fs::create_dir_all(&custom_voices_dir)
            .map_err(|source| AppError::io("creating custom voice library", source))?;
        Ok(Self {
            custom_voices_dir,
            names: RwLock::new(Vec::new()),
        })
    }

    pub fn save_custom_voice(&self, mut profile: CustomVoiceProfile) -> AppResult<CustomVoiceProfile> {
        let voice_name = require_voice_name(&profile.voice_name)?;
        profile.voice_name = voice_name.clone();
        profile.sync_status = SyncStatus::PendingSync;
        if profile.created_at.timestamp_millis() == 0 {
            profile.created_at = Utc::now();
        }
        profile.reference_audio_path = self.store_reference_audio(&voice_name, &profile.reference_audio_path)?;
        self.write_profile(profile)
    }

    pub fn save_custom_voice_wav_bytes(
        &self,
        mut profile: CustomVoiceProfile,
        wav_file_name: &str,
        wav_bytes: &[u8],
    ) -> AppResult<CustomVoiceProfile> {
        require_wav_file_name(wav_file_name)?;
        if wav_bytes.is_empty() {
            return Err(AppError::offline_job("referenceAudioBytes is required"));
        }
        let voice_name = require_voice_name(&profile.voice_name)?;
        profile.voice_name = voice_name.clone();
        profile.sync_status = SyncStatus::PendingSync;
        if profile.created_at.timestamp_millis() == 0 {
            profile.created_at = Utc::now();
        }
        let target_path = self.audio_path(&voice_name)?;
        std::fs::write(&target_path, wav_bytes).map_err(|source| AppError::io("writing custom voice wav", source))?;
        profile.reference_audio_path = target_path.to_string_lossy().into_owned();
        self.write_profile(profile)
    }

    pub fn save_custom_voice_fields(
        &self,
        voice_name: &str,
        voice_instruction: String,
        reference_text: String,
        wav_upload: Option<(&str, &[u8])>,
    ) -> AppResult<CustomVoiceProfile> {
        let voice_name = require_voice_name(voice_name)?;
        let now = Utc::now();
        let mut profile = self
            .get_custom_voice(&voice_name)
            .unwrap_or_else(|_| CustomVoiceProfile {
                voice_name: voice_name.clone(),
                source_prompt_text: None,
                asr_text: None,
                voice_instruction: String::new(),
                reference_audio_path: String::new(),
                reference_text: String::new(),
                sync_status: SyncStatus::LocalOnly,
                last_synced_at: None,
                created_at: now,
            });

        profile.voice_name = voice_name;
        profile.voice_instruction = voice_instruction;
        profile.reference_text = reference_text;

        if let Some((wav_file_name, wav_bytes)) = wav_upload {
            return self.save_custom_voice_wav_bytes(profile, wav_file_name, wav_bytes);
        }

        if profile.reference_audio_path.trim().is_empty() {
            return Err(AppError::offline_job(
                "referenceAudioBytes is required when no existing reference audio is stored",
            ));
        }

        profile.sync_status = SyncStatus::PendingSync;
        self.write_profile(profile)
    }

    pub fn reference_audio_path_for_voice(&self, voice_name: &str) -> AppResult<PathBuf> {
        let profile = self.get_custom_voice(voice_name)?;
        if profile.reference_audio_path.trim().is_empty() {
            return Err(AppError::offline_job(format!(
                "custom voice has no reference audio: {}",
                profile.voice_name
            )));
        }
        Ok(PathBuf::from(profile.reference_audio_path))
    }

    pub fn mark_sync_status(&self, voice_name: &str, sync_status: SyncStatus) -> AppResult<CustomVoiceProfile> {
        let mut profile = self.get_custom_voice(voice_name)?;
        profile.sync_status = sync_status.clone();
        profile.last_synced_at = if sync_status == SyncStatus::Synced {
            Some(Utc::now())
        } else {
            profile.last_synced_at
        };
        self.write_profile(profile)
    }

    pub fn upsert_remote_voice(&self, remote: &RemoteVoiceInfo) -> AppResult<CustomVoiceProfile> {
        let voice_name = require_voice_name(&remote.voice_name)?;
        let now = Utc::now();
        let mut profile = self
            .get_custom_voice(&voice_name)
            .unwrap_or_else(|_| CustomVoiceProfile {
                voice_name: voice_name.clone(),
                source_prompt_text: Some("funspeechRemote".into()),
                asr_text: None,
                voice_instruction: String::new(),
                reference_audio_path: String::new(),
                reference_text: String::new(),
                sync_status: SyncStatus::Synced,
                last_synced_at: Some(now),
                created_at: now,
            });

        if profile.voice_instruction.trim().is_empty() {
            profile.voice_instruction = remote.voice_instruction.clone();
        }
        if profile.reference_audio_path.trim().is_empty() {
            profile.reference_audio_path = remote.reference_audio.clone();
        }
        if profile.reference_text.trim().is_empty() {
            profile.reference_text = remote.reference_text.clone();
        }
        profile.sync_status = SyncStatus::Synced;
        profile.last_synced_at = Some(now);
        self.write_profile(profile)
    }

    pub fn delete_custom_voice(&self, voice_name: &str) -> AppResult<CustomVoiceProfile> {
        let profile = self.get_custom_voice(voice_name)?;
        let path = self.profile_path(&profile.voice_name)?;
        if path.exists() {
            std::fs::remove_file(&path).map_err(|source| AppError::io("deleting custom voice profile", source))?;
        }
        let audio_path = self.audio_path(&profile.voice_name)?;
        if audio_path.exists() {
            std::fs::remove_file(&audio_path).map_err(|source| AppError::io("deleting custom voice audio", source))?;
        }
        self.names
            .write()
            .expect("voice library lock poisoned")
            .retain(|name| name != &profile.voice_name);
        Ok(profile)
    }

    pub fn get_custom_voice(&self, voice_name: &str) -> AppResult<CustomVoiceProfile> {
        let voice_name = require_voice_name(voice_name)?;
        let path = self.profile_path(&voice_name)?;
        if !path.exists() {
            return Err(AppError::offline_job(format!("custom voice not found: {voice_name}")));
        }
        JsonStore::new(path, empty_profile(voice_name)).load_or_create()
    }

    pub fn list_custom_voices(&self) -> AppResult<Vec<CustomVoiceProfile>> {
        let mut profiles = Vec::new();
        for entry in std::fs::read_dir(&self.custom_voices_dir)
            .map_err(|source| AppError::io("reading custom voice library", source))?
        {
            let entry = entry.map_err(|source| AppError::io("reading custom voice entry", source))?;
            if entry.path().extension().and_then(|ext| ext.to_str()) == Some("json") {
                let store = JsonStore::new(entry.path(), empty_profile(String::new()));
                profiles.push(store.load_or_create()?);
            }
        }
        profiles.sort_by(|left, right| left.voice_name.cmp(&right.voice_name));
        Ok(profiles)
    }

    fn write_profile(&self, profile: CustomVoiceProfile) -> AppResult<CustomVoiceProfile> {
        let voice_name = require_voice_name(&profile.voice_name)?;
        let path = self.profile_path(&voice_name)?;
        let store = JsonStore::new(&path, profile.clone());
        let saved = store.replace(profile)?;
        let mut names = self.names.write().expect("voice library lock poisoned");
        if !names.contains(&voice_name) {
            names.push(voice_name);
            names.sort();
        }
        Ok(saved)
    }

    fn profile_path(&self, voice_name: &str) -> AppResult<PathBuf> {
        let safe_name = sanitize_voice_name(voice_name);
        if safe_name.is_empty() {
            return Err(AppError::offline_job("voiceName must contain a safe file name segment"));
        }
        Ok(self.custom_voices_dir.join(format!("{safe_name}.json")))
    }

    fn audio_path(&self, voice_name: &str) -> AppResult<PathBuf> {
        let safe_name = sanitize_voice_name(voice_name);
        if safe_name.is_empty() {
            return Err(AppError::offline_job("voiceName must contain a safe file name segment"));
        }
        Ok(self.custom_voices_dir.join(format!("{safe_name}.wav")))
    }

    fn store_reference_audio(&self, voice_name: &str, source_audio_path: &str) -> AppResult<String> {
        let source_audio_path = require_non_empty("referenceAudioPath", source_audio_path)?;
        let source_path = PathBuf::from(source_audio_path);
        require_wav_file_name(
            source_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default(),
        )?;
        if !source_path.exists() {
            return Err(AppError::offline_job(format!(
                "reference audio file not found: {}",
                source_path.display()
            )));
        }

        let target_path = self.audio_path(voice_name)?;
        if source_path != target_path {
            std::fs::copy(&source_path, &target_path)
                .map_err(|source| AppError::io("copying custom voice audio", source))?;
        }
        Ok(target_path.to_string_lossy().into_owned())
    }
}

fn empty_profile(voice_name: String) -> CustomVoiceProfile {
    CustomVoiceProfile {
        voice_name,
        source_prompt_text: None,
        asr_text: None,
        voice_instruction: String::new(),
        reference_audio_path: String::new(),
        reference_text: String::new(),
        sync_status: SyncStatus::LocalOnly,
        last_synced_at: None,
        created_at: Utc::now(),
    }
}

fn require_voice_name(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::offline_job("voiceName is required"))
    } else {
        Ok(trimmed.to_string())
    }
}

fn require_non_empty(field: &str, value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::offline_job(format!("{field} is required")))
    } else {
        Ok(trimmed.to_string())
    }
}

fn require_wav_file_name(value: &str) -> AppResult<()> {
    if PathBuf::from(value)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("wav"))
        == Some(true)
    {
        Ok(())
    } else {
        Err(AppError::offline_job("reference audio must be a .wav file"))
    }
}

fn sanitize_voice_name(value: &str) -> String {
    let mut safe_name = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            safe_name.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() {
            continue;
        } else {
            safe_name.push_str(&format!("_x{:x}", ch as u32));
        }
    }
    safe_name
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use chrono::Utc;

    use crate::domain::voice::{CustomVoiceProfile, SyncStatus};

    use super::VoiceLibrary;

    fn temp_root(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("voice-cloner-{name}-{unique}"))
    }

    fn library() -> VoiceLibrary {
        VoiceLibrary::new(temp_root("library")).unwrap()
    }

    fn reference_audio_path() -> String {
        let source_dir = temp_root("source-audio");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("preview.wav");
        std::fs::write(&path, b"fake wav").unwrap();
        path.to_string_lossy().into_owned()
    }

    fn profile() -> CustomVoiceProfile {
        CustomVoiceProfile {
            voice_name: "My Voice".into(),
            source_prompt_text: Some("warm narrator".into()),
            asr_text: None,
            voice_instruction: "warm, calm".into(),
            reference_audio_path: reference_audio_path(),
            reference_text: "hello".into(),
            sync_status: SyncStatus::LocalOnly,
            last_synced_at: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn voice_library_saves_and_lists_custom_voice_profiles() {
        let library = library();
        let saved = library.save_custom_voice(profile()).unwrap();

        assert_eq!(saved.sync_status, SyncStatus::PendingSync);
        assert!(saved.reference_audio_path.ends_with("myvoice.wav"));
        assert_eq!(std::fs::read(&saved.reference_audio_path).unwrap(), b"fake wav");
        assert_eq!(
            library.get_custom_voice("My Voice").unwrap().voice_instruction,
            "warm, calm"
        );
        assert_eq!(library.list_custom_voices().unwrap().len(), 1);
    }

    #[test]
    fn voice_library_marks_sync_status_and_deletes_profiles() {
        let library = library();
        library.save_custom_voice(profile()).unwrap();

        let synced = library.mark_sync_status("My Voice", SyncStatus::Synced).unwrap();
        assert_eq!(synced.sync_status, SyncStatus::Synced);
        assert!(synced.last_synced_at.is_some());

        let deleted = library.delete_custom_voice("My Voice").unwrap();
        assert_eq!(deleted.voice_name, "My Voice");
        assert!(!std::path::PathBuf::from(deleted.reference_audio_path).exists());
        assert!(library.list_custom_voices().unwrap().is_empty());
    }

    #[test]
    fn voice_library_updates_text_fields_without_requiring_frontend_audio_path() {
        let library = library();
        let saved = library.save_custom_voice(profile()).unwrap();

        let updated = library
            .save_custom_voice_fields("My Voice", "brighter".into(), "updated reference text".into(), None)
            .unwrap();

        assert_eq!(updated.voice_instruction, "brighter");
        assert_eq!(updated.reference_text, "updated reference text");
        assert_eq!(updated.reference_audio_path, saved.reference_audio_path);
        assert!(std::path::PathBuf::from(updated.reference_audio_path).exists());
    }

    #[test]
    fn voice_library_supports_non_ascii_voice_names_with_safe_file_segments() {
        let library = library();
        let mut profile = profile();
        profile.voice_name = "中文女".into();

        let saved = library.save_custom_voice(profile).unwrap();

        assert_eq!(saved.voice_name, "中文女");
        assert!(saved.reference_audio_path.ends_with("_x4e2d_x6587_x5973.wav"));
        assert_eq!(
            library.get_custom_voice("中文女").unwrap().voice_instruction,
            "warm, calm"
        );
    }

    #[test]
    fn voice_library_rejects_missing_reference_audio() {
        let library = library();
        let mut profile = profile();
        profile.reference_audio_path = "missing-preview.wav".into();

        let error = library.save_custom_voice(profile).unwrap_err().to_string();

        assert!(error.contains("reference audio file not found"));
    }
}
