use std::{collections::BTreeMap, path::PathBuf, sync::RwLock};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    app::{
        error::{AppError, AppResult},
        trace::{new_entity_id, TraceId},
    },
    clients::funspeech::{
        offline::OfflineEndpoints,
        tts::{synthesize_text, OfflineTtsRequest},
    },
    domain::{
        offline_job::{OfflineJob, OfflineJobInputType, OfflineJobStatus},
        runtime_params::RuntimeParams,
        settings::AppSettings,
    },
    services::asset_cache::AssetCache,
};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateOfflineAudioJobRequest {
    pub input_ref: String,
    pub voice_name: String,
    #[serde(default)]
    pub runtime_params: RuntimeParams,
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateOfflineTextJobRequest {
    pub text: String,
    pub voice_name: String,
    #[serde(default)]
    pub runtime_params: RuntimeParams,
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompleteOfflineJobRequest {
    pub artifact_url: Option<String>,
    pub local_artifact_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FailOfflineJobRequest {
    pub message: String,
}

#[derive(Debug, Default)]
pub struct OfflineJobManager {
    jobs: RwLock<BTreeMap<String, OfflineJob>>,
}

impl OfflineJobManager {
    pub fn create_audio_job(
        &self,
        request: CreateOfflineAudioJobRequest,
        settings: &AppSettings,
    ) -> AppResult<OfflineJob> {
        let input_ref = require_non_empty("inputRef", request.input_ref)?;
        self.create_job(
            OfflineJobInputType::Audio,
            input_ref,
            request.voice_name,
            request.runtime_params,
            request.output_format,
            settings,
        )
    }

    pub fn create_text_job(
        &self,
        request: CreateOfflineTextJobRequest,
        settings: &AppSettings,
    ) -> AppResult<OfflineJob> {
        let text = require_non_empty("text", request.text)?;
        self.create_job(
            OfflineJobInputType::Text,
            text,
            request.voice_name,
            request.runtime_params,
            request.output_format,
            settings,
        )
    }

    fn create_job(
        &self,
        input_type: OfflineJobInputType,
        input_ref: String,
        voice_name: String,
        runtime_params: RuntimeParams,
        output_format: Option<String>,
        settings: &AppSettings,
    ) -> AppResult<OfflineJob> {
        let voice_name = require_non_empty("voiceName", voice_name)?;
        settings.validate().map_err(AppError::invalid_settings)?;
        let endpoints = OfflineEndpoints::from_backend_configs(&settings.backend.asr, &settings.backend.tts);
        let now = Utc::now();
        let job = OfflineJob {
            job_id: new_entity_id("offline"),
            trace_id: TraceId::new("offline").into_string(),
            input_type: input_type.clone(),
            input_ref,
            voice_name,
            runtime_params,
            output_format: normalize_output_format(output_format, &settings.runtime.default_output_format)?,
            status: OfflineJobStatus::Created,
            artifact_url: Some(default_submission_endpoint(&endpoints, &input_type)),
            local_artifact_path: None,
            error_summary: None,
            created_at: now,
            updated_at: now,
        };

        self.jobs
            .write()
            .expect("offline job manager lock poisoned")
            .insert(job.job_id.clone(), job.clone());
        Ok(job)
    }

    pub fn start_job(&self, job_id: &str, settings: &AppSettings, cache: &AssetCache) -> AppResult<OfflineJob> {
        let running = self.transition(job_id, OfflineJobStatus::Running, None)?;
        if running.input_type != OfflineJobInputType::Text {
            return Ok(running);
        }

        match synthesize_text(
            &settings.backend.tts,
            OfflineTtsRequest {
                text: running.input_ref.clone(),
                voice_name: running.voice_name.clone(),
                runtime_params: running.runtime_params.clone(),
                output_format: running.output_format.clone(),
                sample_rate: settings.runtime.default_sample_rate,
            },
        ) {
            Ok(result) => {
                let artifact =
                    cache.write_offline_artifact_bytes(&running.job_id, &running.output_format, &result.audio_bytes)?;
                let mut jobs = self.jobs.write().expect("offline job manager lock poisoned");
                let job = jobs
                    .get_mut(job_id)
                    .ok_or_else(|| AppError::offline_job(format!("offline job not found: {job_id}")))?;
                job.local_artifact_path = Some(artifact.path.to_string_lossy().into_owned());
                job.error_summary = None;
                job.transition_to(OfflineJobStatus::Completed);
                Ok(job.clone())
            }
            Err(error) => {
                let mut jobs = self.jobs.write().expect("offline job manager lock poisoned");
                let job = jobs
                    .get_mut(job_id)
                    .ok_or_else(|| AppError::offline_job(format!("offline job not found: {job_id}")))?;
                job.error_summary = Some(error.to_string());
                job.transition_to(OfflineJobStatus::Failed);
                Ok(job.clone())
            }
        }
    }

    pub fn cancel_job(&self, job_id: &str) -> AppResult<OfflineJob> {
        self.transition(job_id, OfflineJobStatus::Cancelled, None)
    }

    pub fn retry_job(&self, job_id: &str) -> AppResult<OfflineJob> {
        let mut jobs = self.jobs.write().expect("offline job manager lock poisoned");
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| AppError::offline_job(format!("offline job not found: {job_id}")))?;

        match job.status {
            OfflineJobStatus::Failed | OfflineJobStatus::Cancelled => {
                job.error_summary = None;
                job.local_artifact_path = None;
                job.transition_to(OfflineJobStatus::Created);
                Ok(job.clone())
            }
            _ => Err(AppError::offline_job(format!(
                "cannot retry offline job from {:?}",
                job.status
            ))),
        }
    }

    pub fn complete_job(
        &self,
        job_id: &str,
        request: CompleteOfflineJobRequest,
        cache: &AssetCache,
    ) -> AppResult<OfflineJob> {
        let mut jobs = self.jobs.write().expect("offline job manager lock poisoned");
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| AppError::offline_job(format!("offline job not found: {job_id}")))?;

        if let Some(artifact_url) = request.artifact_url {
            job.artifact_url = Some(artifact_url);
        }
        if let Some(path) = request.local_artifact_path {
            let artifact = cache.register_existing_artifact(job_id, &job.output_format, PathBuf::from(path))?;
            job.local_artifact_path = Some(artifact.path.to_string_lossy().into_owned());
        } else {
            let artifact = cache.offline_artifact_path(job_id, &job.output_format)?;
            job.local_artifact_path = Some(artifact.path.to_string_lossy().into_owned());
        }
        job.error_summary = None;
        job.transition_to(OfflineJobStatus::Completed);
        Ok(job.clone())
    }

    pub fn fail_job(&self, job_id: &str, request: FailOfflineJobRequest) -> AppResult<OfflineJob> {
        self.transition(
            job_id,
            OfflineJobStatus::Failed,
            Some(require_non_empty("message", request.message)?),
        )
    }

    pub fn get_job(&self, job_id: &str) -> AppResult<OfflineJob> {
        self.jobs
            .read()
            .expect("offline job manager lock poisoned")
            .get(job_id)
            .cloned()
            .ok_or_else(|| AppError::offline_job(format!("offline job not found: {job_id}")))
    }

    pub fn list_jobs(&self) -> Vec<OfflineJob> {
        self.jobs
            .read()
            .expect("offline job manager lock poisoned")
            .values()
            .cloned()
            .collect()
    }

    fn transition(
        &self,
        job_id: &str,
        status: OfflineJobStatus,
        error_summary: Option<String>,
    ) -> AppResult<OfflineJob> {
        let mut jobs = self.jobs.write().expect("offline job manager lock poisoned");
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| AppError::offline_job(format!("offline job not found: {job_id}")))?;

        job.error_summary = error_summary;
        job.transition_to(status);
        Ok(job.clone())
    }
}

fn require_non_empty(field: &str, value: String) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::offline_job(format!("{field} is required")))
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_output_format(format: Option<String>, fallback: &str) -> AppResult<String> {
    let value = format.unwrap_or_else(|| fallback.to_string());
    let normalized = value.trim().trim_start_matches('.').to_ascii_lowercase();
    if normalized.is_empty() {
        Err(AppError::offline_job("outputFormat is required"))
    } else {
        Ok(normalized)
    }
}

fn default_submission_endpoint(endpoints: &OfflineEndpoints, input_type: &OfflineJobInputType) -> String {
    match input_type {
        OfflineJobInputType::Audio => endpoints.asr_url.clone(),
        OfflineJobInputType::Text => endpoints.tts_url.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{
        domain::{offline_job::OfflineJobStatus, runtime_params::RuntimeParams, settings::AppSettings},
        services::asset_cache::AssetCache,
    };

    use super::{
        CompleteOfflineJobRequest, CreateOfflineAudioJobRequest, CreateOfflineTextJobRequest, FailOfflineJobRequest,
        OfflineJobManager,
    };

    fn cache() -> AssetCache {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        AssetCache::new(
            std::env::temp_dir().join(format!("voice-cloner-offline-jobs-{unique}/offline")),
            std::env::temp_dir().join(format!("voice-cloner-offline-jobs-{unique}/voice-design")),
        )
        .unwrap()
    }

    #[test]
    fn offline_manager_creates_audio_and_text_jobs_with_funspeech_endpoint_hints() {
        let manager = OfflineJobManager::default();
        let settings = AppSettings::default();

        let audio = manager
            .create_audio_job(
                CreateOfflineAudioJobRequest {
                    input_ref: "C:/recordings/input.wav".into(),
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: Some("WAV".into()),
                },
                &settings,
            )
            .unwrap();
        let text = manager
            .create_text_job(
                CreateOfflineTextJobRequest {
                    text: "hello".into(),
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: None,
                },
                &settings,
            )
            .unwrap();

        assert_eq!(audio.status, OfflineJobStatus::Created);
        assert!(audio.artifact_url.unwrap().ends_with("/stream/v1/asr"));
        assert_eq!(audio.output_format, "wav");
        assert!(text.artifact_url.unwrap().ends_with("/stream/v1/tts"));
        assert_eq!(manager.list_jobs().len(), 2);
    }

    #[test]
    fn offline_manager_runs_fails_retries_and_completes_jobs() {
        let manager = OfflineJobManager::default();
        let settings = AppSettings::default();
        let cache = cache();
        let created = manager
            .create_audio_job(
                CreateOfflineAudioJobRequest {
                    input_ref: "C:/recordings/input.wav".into(),
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: Some("mp3".into()),
                },
                &settings,
            )
            .unwrap();

        assert_eq!(
            manager.start_job(&created.job_id, &settings, &cache).unwrap().status,
            OfflineJobStatus::Running
        );
        let failed = manager
            .fail_job(
                &created.job_id,
                FailOfflineJobRequest {
                    message: "backend failed".into(),
                },
            )
            .unwrap();
        assert_eq!(failed.status, OfflineJobStatus::Failed);
        assert_eq!(failed.error_summary.as_deref(), Some("backend failed"));

        let retried = manager.retry_job(&created.job_id).unwrap();
        assert_eq!(retried.status, OfflineJobStatus::Created);
        assert!(retried.error_summary.is_none());

        let completed = manager
            .complete_job(
                &created.job_id,
                CompleteOfflineJobRequest {
                    artifact_url: Some("https://voice.example.com/out.mp3".into()),
                    local_artifact_path: None,
                },
                &cache,
            )
            .unwrap();
        assert_eq!(completed.status, OfflineJobStatus::Completed);
        assert!(completed.local_artifact_path.unwrap().ends_with(".mp3"));
    }

    #[test]
    fn offline_manager_rejects_empty_text_jobs() {
        let error = OfflineJobManager::default()
            .create_text_job(
                CreateOfflineTextJobRequest {
                    text: " ".into(),
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: None,
                },
                &AppSettings::default(),
            )
            .unwrap_err();

        assert!(error.to_string().contains("text"));
    }
}
