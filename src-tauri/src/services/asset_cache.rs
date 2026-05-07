use std::path::PathBuf;

use serde::Serialize;

use crate::app::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OfflineArtifactPath {
    pub job_id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDesignArtifactPath {
    pub draft_id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AssetCache {
    offline_exports_dir: PathBuf,
    voice_design_artifacts_dir: PathBuf,
}

impl AssetCache {
    pub fn new(
        offline_exports_dir: impl Into<PathBuf>,
        voice_design_artifacts_dir: impl Into<PathBuf>,
    ) -> AppResult<Self> {
        let offline_exports_dir = offline_exports_dir.into();
        let voice_design_artifacts_dir = voice_design_artifacts_dir.into();
        std::fs::create_dir_all(&offline_exports_dir)
            .map_err(|source| AppError::io("creating offline exports cache", source))?;
        std::fs::create_dir_all(&voice_design_artifacts_dir)
            .map_err(|source| AppError::io("creating voice design artifacts cache", source))?;
        Ok(Self {
            offline_exports_dir,
            voice_design_artifacts_dir,
        })
    }

    pub fn offline_artifact_path(&self, job_id: &str, output_format: &str) -> AppResult<OfflineArtifactPath> {
        let safe_job_id = sanitize_path_segment(job_id);
        let safe_format = sanitize_path_segment(output_format.trim_start_matches('.'));
        if safe_job_id.is_empty() {
            return Err(AppError::invalid_settings(
                "job id is required for offline artifact path",
            ));
        }
        if safe_format.is_empty() {
            return Err(AppError::invalid_settings(
                "output format is required for offline artifact path",
            ));
        }

        Ok(OfflineArtifactPath {
            job_id: job_id.to_string(),
            path: self.offline_exports_dir.join(format!("{safe_job_id}.{safe_format}")),
        })
    }

    pub fn register_existing_artifact(
        &self,
        job_id: &str,
        output_format: &str,
        source_path: impl Into<PathBuf>,
    ) -> AppResult<OfflineArtifactPath> {
        let artifact = self.offline_artifact_path(job_id, output_format)?;
        copy_if_needed(
            source_path.into(),
            &artifact.path,
            "copying offline artifact into cache",
        )?;
        Ok(artifact)
    }

    pub fn voice_design_artifact_path(
        &self,
        draft_id: &str,
        output_format: &str,
    ) -> AppResult<VoiceDesignArtifactPath> {
        let safe_draft_id = sanitize_path_segment(draft_id);
        let safe_format = sanitize_path_segment(output_format.trim_start_matches('.'));
        if safe_draft_id.is_empty() {
            return Err(AppError::invalid_settings(
                "draft id is required for voice design artifact path",
            ));
        }
        if safe_format.is_empty() {
            return Err(AppError::invalid_settings(
                "output format is required for voice design artifact path",
            ));
        }

        Ok(VoiceDesignArtifactPath {
            draft_id: draft_id.to_string(),
            path: self
                .voice_design_artifacts_dir
                .join(format!("{safe_draft_id}.{safe_format}")),
        })
    }

    pub fn register_voice_design_artifact(
        &self,
        draft_id: &str,
        output_format: &str,
        source_path: impl Into<PathBuf>,
    ) -> AppResult<VoiceDesignArtifactPath> {
        let artifact = self.voice_design_artifact_path(draft_id, output_format)?;
        copy_if_needed(
            source_path.into(),
            &artifact.path,
            "copying voice design artifact into cache",
        )?;
        Ok(artifact)
    }
}

fn copy_if_needed(source_path: PathBuf, target_path: &PathBuf, context: &'static str) -> AppResult<()> {
    if source_path != *target_path {
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| AppError::io("creating artifact directory", source))?;
        }
        std::fs::copy(&source_path, target_path).map_err(|source| AppError::io(context, source))?;
    }
    Ok(())
}

fn sanitize_path_segment(value: &str) -> String {
    value
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                Some(ch.to_ascii_lowercase())
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::AssetCache;

    fn temp_cache_dirs() -> (std::path::PathBuf, std::path::PathBuf) {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("voice-cloner-cache-{unique}"));
        (root.join("offline"), root.join("voice-design"))
    }

    fn cache() -> AssetCache {
        let (offline, voice_design) = temp_cache_dirs();
        AssetCache::new(offline, voice_design).unwrap()
    }

    #[test]
    fn asset_cache_builds_safe_offline_export_paths() {
        let cache = cache();
        let artifact = cache.offline_artifact_path("job-123", "WAV").unwrap();

        assert!(artifact.path.ends_with("job-123.wav"));
    }

    #[test]
    fn asset_cache_copies_existing_artifact_into_export_cache() {
        let (offline, voice_design) = temp_cache_dirs();
        let source_dir = offline.parent().unwrap().join("source");
        let source = source_dir.join("source.wav");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(&source, b"fake wav").unwrap();
        let cache = AssetCache::new(offline, voice_design).unwrap();

        let artifact = cache.register_existing_artifact("job-1", "wav", &source).unwrap();

        assert_eq!(std::fs::read(artifact.path).unwrap(), b"fake wav");
    }

    #[test]
    fn asset_cache_builds_and_copies_voice_design_artifacts() {
        let (offline, voice_design) = temp_cache_dirs();
        let source_dir = voice_design.parent().unwrap().join("source");
        let source = source_dir.join("preview.wav");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(&source, b"preview wav").unwrap();
        let cache = AssetCache::new(offline, voice_design).unwrap();

        let artifact = cache.register_voice_design_artifact("draft-1", "WAV", &source).unwrap();

        assert!(artifact.path.ends_with("draft-1.wav"));
        assert_eq!(std::fs::read(artifact.path).unwrap(), b"preview wav");
    }
}
