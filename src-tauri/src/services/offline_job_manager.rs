use std::{
    collections::BTreeMap,
    io::Cursor,
    path::{Path, PathBuf},
    sync::RwLock,
};

use chrono::Utc;
use hound::{SampleFormat, WavSpec};
use serde::{Deserialize, Serialize};

use crate::{
    app::{
        error::{AppError, AppResult},
        trace::{new_entity_id, TraceId},
    },
    audio::normalizer::{normalize_wav_bytes, AudioNormalizationConfig},
    clients::funspeech::{
        offline::{transcribe_audio_bytes, transcribe_audio_bytes_async, OfflineEndpoints},
        tts::{synthesize_text, OfflineTtsRequest},
    },
    domain::{
        offline_job::{OfflineJob, OfflineJobInputType, OfflineJobStatus},
        runtime_params::RuntimeParams,
        settings::AppSettings,
    },
    services::asset_cache::AssetCache,
    storage::json_store::JsonStore,
};

const OFFLINE_AUDIO_CHUNK_SECONDS: usize = 60;
const SHORT_AUDIO_DIRECT_ASR_SECONDS: f64 = 10.0;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateOfflineAudioJobRequest {
    #[serde(default)]
    pub input_ref: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub input_bytes: Option<Vec<u8>>,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OfflineJobStoreDoc {
    jobs: Vec<OfflineJob>,
}

impl OfflineJobStoreDoc {
    fn empty() -> Self {
        Self { jobs: Vec::new() }
    }
}

#[derive(Debug)]
pub struct OfflineJobManager {
    jobs: RwLock<BTreeMap<String, OfflineJob>>,
    store: Option<JsonStore<OfflineJobStoreDoc>>,
}

impl Default for OfflineJobManager {
    fn default() -> Self {
        Self {
            jobs: RwLock::new(BTreeMap::new()),
            store: None,
        }
    }
}

impl OfflineJobManager {
    pub fn new(store_path: impl Into<PathBuf>) -> AppResult<Self> {
        let store = JsonStore::new(store_path, OfflineJobStoreDoc::empty());
        let loaded = store.load_or_create()?;
        let jobs = loaded.jobs.into_iter().map(|job| (job.job_id.clone(), job)).collect();
        Ok(Self {
            jobs: RwLock::new(jobs),
            store: Some(store),
        })
    }

    pub fn create_audio_job(
        &self,
        request: CreateOfflineAudioJobRequest,
        settings: &AppSettings,
        cache: &AssetCache,
    ) -> AppResult<OfflineJob> {
        settings.validate().map_err(AppError::invalid_settings)?;
        let job_id = new_entity_id("offline");
        let (input_ref, input_file_name) = prepare_audio_input(
            &job_id,
            request.input_ref,
            request.file_name,
            request.input_bytes,
            cache,
        )?;
        self.create_job(
            job_id,
            OfflineJobInputType::Audio,
            input_ref,
            input_file_name,
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
        settings.validate().map_err(AppError::invalid_settings)?;
        self.create_job(
            new_entity_id("offline"),
            OfflineJobInputType::Text,
            text,
            None,
            request.voice_name,
            request.runtime_params,
            request.output_format,
            settings,
        )
    }

    fn create_job(
        &self,
        job_id: String,
        input_type: OfflineJobInputType,
        input_ref: String,
        input_file_name: Option<String>,
        voice_name: String,
        runtime_params: RuntimeParams,
        output_format: Option<String>,
        settings: &AppSettings,
    ) -> AppResult<OfflineJob> {
        let voice_name = require_non_empty("voiceName", voice_name)?;
        let endpoints = OfflineEndpoints::from_backend_configs(&settings.backend.asr, &settings.backend.tts);
        let now = Utc::now();
        let job = OfflineJob {
            job_id,
            trace_id: TraceId::new("offline").into_string(),
            input_type: input_type.clone(),
            input_ref,
            input_file_name,
            voice_name,
            runtime_params,
            output_format: normalize_output_format(output_format, &settings.runtime.default_output_format)?,
            status: OfflineJobStatus::Created,
            stage: "created".into(),
            progress: 0,
            artifact_url: Some(default_submission_endpoint(&endpoints, &input_type)),
            local_artifact_path: None,
            error_summary: None,
            created_at: now,
            updated_at: now,
        };

        let mut jobs = self.jobs.write().expect("offline job manager lock poisoned");
        jobs.insert(job.job_id.clone(), job.clone());
        self.persist_locked(&jobs)?;
        Ok(job)
    }

    pub fn begin_job(&self, job_id: &str) -> AppResult<OfflineJob> {
        self.patch_job(job_id, |job| {
            job.status = OfflineJobStatus::Running;
            job.stage = "preparing".into();
            job.progress = 5;
            job.error_summary = None;
            job.updated_at = Utc::now();
        })
    }

    pub fn start_job(&self, job_id: &str, settings: &AppSettings, cache: &AssetCache) -> AppResult<OfflineJob> {
        let running = self.begin_job(job_id)?;
        self.run_started_job_with_updates(running, settings, cache, |_| {})
    }

    pub fn run_started_job_with_updates<F>(
        &self,
        running: OfflineJob,
        settings: &AppSettings,
        cache: &AssetCache,
        on_update: F,
    ) -> AppResult<OfflineJob>
    where
        F: Fn(OfflineJob),
    {
        let result = match running.input_type {
            OfflineJobInputType::Text => self.run_text_job(&running, settings, cache, &on_update),
            OfflineJobInputType::Audio => self.run_audio_job(&running, settings, cache, &on_update),
        };

        match result {
            Ok(job) => Ok(job),
            Err(error) => {
                let failed = self.fail_running_job(&running.job_id, error.to_string())?;
                on_update(failed.clone());
                Ok(failed)
            }
        }
    }

    fn run_audio_job<F>(
        &self,
        running: &OfflineJob,
        settings: &AppSettings,
        cache: &AssetCache,
        on_update: &F,
    ) -> AppResult<OfflineJob>
    where
        F: Fn(OfflineJob),
    {
        let input_duration = wav_duration_seconds(&running.input_ref)?;
        if input_duration <= SHORT_AUDIO_DIRECT_ASR_SECONDS {
            return self.run_short_audio_job(running, settings, cache, on_update);
        }

        self.patch_job_and_emit(
            &running.job_id,
            |job| {
                job.stage = "splittingAudio".into();
                job.progress = 10;
                job.updated_at = Utc::now();
            },
            on_update,
        )?;
        let input_chunks = split_wav_file(&running.input_ref, OFFLINE_AUDIO_CHUNK_SECONDS)?;
        let total_chunks = input_chunks.len();
        let mut output_chunks = Vec::with_capacity(total_chunks);

        for (index, audio_bytes) in input_chunks.into_iter().enumerate() {
            if let Some(cancelled) = self.cancelled_job(&running.job_id)? {
                return Ok(cancelled);
            }
            self.patch_job_and_emit(
                &running.job_id,
                |job| {
                    job.stage = format!("transcribingChunk:{}/{}", index + 1, total_chunks);
                    job.progress = chunk_progress(index, total_chunks, 0);
                    job.updated_at = Utc::now();
                },
                on_update,
            )?;
            let text = transcribe_audio_bytes_async(&settings.backend.asr, &audio_bytes, "wav")?;

            if let Some(cancelled) = self.cancelled_job(&running.job_id)? {
                return Ok(cancelled);
            }
            self.patch_job_and_emit(
                &running.job_id,
                |job| {
                    job.stage = format!("synthesizingChunk:{}/{}", index + 1, total_chunks);
                    job.progress = chunk_progress(index, total_chunks, 1);
                    job.updated_at = Utc::now();
                },
                on_update,
            )?;
            let result = synthesize_text(
                &settings.backend.tts,
                OfflineTtsRequest {
                    text,
                    voice_name: running.voice_name.clone(),
                    runtime_params: running.runtime_params.clone(),
                    output_format: running.output_format.clone(),
                    sample_rate: settings.runtime.default_sample_rate,
                },
            )?;
            ensure_wav_audio(
                &running.output_format,
                &result.audio_bytes,
                result.content_type.as_deref(),
            )?;
            output_chunks.push(result.audio_bytes);
        }

        if let Some(cancelled) = self.cancelled_job(&running.job_id)? {
            return Ok(cancelled);
        }
        self.patch_job_and_emit(
            &running.job_id,
            |job| {
                job.stage = "mergingChunks".into();
                job.progress = 88;
                job.updated_at = Utc::now();
            },
            on_update,
        )?;
        let merged_audio = merge_wav_chunks(&output_chunks)?;
        self.complete_with_audio_bytes(running, cache, &merged_audio, on_update)
    }

    fn run_short_audio_job<F>(
        &self,
        running: &OfflineJob,
        settings: &AppSettings,
        cache: &AssetCache,
        on_update: &F,
    ) -> AppResult<OfflineJob>
    where
        F: Fn(OfflineJob),
    {
        self.patch_job_and_emit(
            &running.job_id,
            |job| {
                job.stage = "transcribing".into();
                job.progress = 20;
                job.updated_at = Utc::now();
            },
            on_update,
        )?;
        let audio_bytes =
            std::fs::read(&running.input_ref).map_err(|source| AppError::io("reading offline audio input", source))?;
        let text = transcribe_audio_bytes(&settings.backend.asr, &audio_bytes, "wav")?;

        if let Some(cancelled) = self.cancelled_job(&running.job_id)? {
            return Ok(cancelled);
        }
        self.patch_job_and_emit(
            &running.job_id,
            |job| {
                job.stage = "synthesizing".into();
                job.progress = 60;
                job.updated_at = Utc::now();
            },
            on_update,
        )?;
        let result = synthesize_text(
            &settings.backend.tts,
            OfflineTtsRequest {
                text,
                voice_name: running.voice_name.clone(),
                runtime_params: running.runtime_params.clone(),
                output_format: running.output_format.clone(),
                sample_rate: settings.runtime.default_sample_rate,
            },
        )?;
        ensure_wav_audio(
            &running.output_format,
            &result.audio_bytes,
            result.content_type.as_deref(),
        )?;
        self.complete_with_audio_bytes(running, cache, &result.audio_bytes, on_update)
    }

    fn run_text_job<F>(
        &self,
        running: &OfflineJob,
        settings: &AppSettings,
        cache: &AssetCache,
        on_update: &F,
    ) -> AppResult<OfflineJob>
    where
        F: Fn(OfflineJob),
    {
        self.patch_job_and_emit(
            &running.job_id,
            |job| {
                job.stage = "synthesizing".into();
                job.progress = 45;
                job.updated_at = Utc::now();
            },
            on_update,
        )?;
        let result = synthesize_text(
            &settings.backend.tts,
            OfflineTtsRequest {
                text: running.input_ref.clone(),
                voice_name: running.voice_name.clone(),
                runtime_params: running.runtime_params.clone(),
                output_format: running.output_format.clone(),
                sample_rate: settings.runtime.default_sample_rate,
            },
        )?;
        ensure_wav_audio(
            &running.output_format,
            &result.audio_bytes,
            result.content_type.as_deref(),
        )?;
        self.complete_with_audio_bytes(running, cache, &result.audio_bytes, on_update)
    }

    fn complete_with_audio_bytes<F>(
        &self,
        running: &OfflineJob,
        cache: &AssetCache,
        audio_bytes: &[u8],
        on_update: &F,
    ) -> AppResult<OfflineJob>
    where
        F: Fn(OfflineJob),
    {
        if let Some(cancelled) = self.cancelled_job(&running.job_id)? {
            return Ok(cancelled);
        }
        let audio_bytes = if running.output_format.eq_ignore_ascii_case("wav") {
            self.patch_job_and_emit(
                &running.job_id,
                |job| {
                    job.stage = "normalizingAudio".into();
                    job.progress = 89;
                    job.updated_at = Utc::now();
                },
                on_update,
            )?;
            normalize_offline_wav_bytes(audio_bytes)?
        } else {
            audio_bytes.to_vec()
        };
        if let Some(cancelled) = self.cancelled_job(&running.job_id)? {
            return Ok(cancelled);
        }
        self.patch_job_and_emit(
            &running.job_id,
            |job| {
                job.stage = "writingArtifact".into();
                job.progress = 90;
                job.updated_at = Utc::now();
            },
            on_update,
        )?;
        let artifact = cache.write_offline_artifact_bytes(&running.job_id, &running.output_format, &audio_bytes)?;
        self.patch_job_and_emit(
            &running.job_id,
            |job| {
                job.local_artifact_path = Some(artifact.path.to_string_lossy().into_owned());
                job.error_summary = None;
                job.status = OfflineJobStatus::Completed;
                job.stage = "completed".into();
                job.progress = 100;
                job.updated_at = Utc::now();
            },
            on_update,
        )
    }

    fn fail_running_job(&self, job_id: &str, message: String) -> AppResult<OfflineJob> {
        if let Some(cancelled) = self.cancelled_job(job_id)? {
            return Ok(cancelled);
        }
        self.patch_job(job_id, |job| {
            job.error_summary = Some(message);
            job.status = OfflineJobStatus::Failed;
            job.stage = "failed".into();
            job.updated_at = Utc::now();
        })
    }

    fn cancelled_job(&self, job_id: &str) -> AppResult<Option<OfflineJob>> {
        let job = self.get_job(job_id)?;
        Ok((job.status == OfflineJobStatus::Cancelled).then_some(job))
    }

    pub fn cancel_job(&self, job_id: &str) -> AppResult<OfflineJob> {
        self.patch_job(job_id, |job| {
            job.status = OfflineJobStatus::Cancelled;
            job.stage = "cancelled".into();
            job.updated_at = Utc::now();
        })
    }

    pub fn retry_job(&self, job_id: &str, cache: &AssetCache) -> AppResult<OfflineJob> {
        let current = self.get_job(job_id)?;
        if !matches!(
            current.status,
            OfflineJobStatus::Failed | OfflineJobStatus::Cancelled | OfflineJobStatus::Completed
        ) {
            return Err(AppError::offline_job(format!(
                "cannot retry offline job from {:?}",
                current.status
            )));
        }
        remove_file_if_present(current.local_artifact_path.as_deref())?;
        let artifact = cache.offline_artifact_path(&current.job_id, &current.output_format)?;
        remove_file_if_present(artifact.path.to_str())?;
        self.patch_job(job_id, |job| {
            job.error_summary = None;
            job.local_artifact_path = None;
            job.status = OfflineJobStatus::Created;
            job.stage = "created".into();
            job.progress = 0;
            job.updated_at = Utc::now();
        })
    }

    pub fn clear_jobs(&self, cache: &AssetCache) -> AppResult<Vec<OfflineJob>> {
        let mut jobs = self.jobs.write().expect("offline job manager lock poisoned");
        let removed = jobs.values().cloned().collect();
        jobs.clear();
        self.persist_locked(&jobs)?;
        cache.clear_offline_audio_files()?;
        Ok(removed)
    }

    pub fn delete_job(&self, job_id: &str, cache: &AssetCache) -> AppResult<OfflineJob> {
        let mut jobs = self.jobs.write().expect("offline job manager lock poisoned");
        let removed = jobs
            .remove(job_id)
            .ok_or_else(|| AppError::offline_job(format!("offline job not found: {job_id}")))?;
        self.persist_locked(&jobs)?;
        drop(jobs);

        remove_file_if_present(removed.local_artifact_path.as_deref())?;
        let artifact = cache.offline_artifact_path(&removed.job_id, &removed.output_format)?;
        remove_file_if_present(artifact.path.to_str())?;
        if matches!(removed.input_type, OfflineJobInputType::Audio) {
            remove_file_if_present(Some(&removed.input_ref))?;
        }
        Ok(removed)
    }

    pub fn copy_artifact_to(&self, job_id: &str, target_path: impl Into<PathBuf>) -> AppResult<PathBuf> {
        let job = self.get_job(job_id)?;
        if job.status != OfflineJobStatus::Completed {
            return Err(AppError::offline_job("only completed offline jobs can be downloaded"));
        }
        let source_path = job
            .local_artifact_path
            .ok_or_else(|| AppError::offline_job("completed offline job has no local artifact"))?;
        let target_path = target_path.into();
        if target_path.as_os_str().is_empty() {
            return Err(AppError::offline_job("download target path is required"));
        }
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|source| AppError::io("creating download target directory", source))?;
        }
        std::fs::copy(&source_path, &target_path).map_err(|source| AppError::io("copying offline artifact", source))?;
        Ok(target_path)
    }

    pub fn complete_job(
        &self,
        job_id: &str,
        request: CompleteOfflineJobRequest,
        cache: &AssetCache,
    ) -> AppResult<OfflineJob> {
        let current = self.get_job(job_id)?;
        let artifact_path = if let Some(path) = request.local_artifact_path {
            cache
                .register_existing_artifact(job_id, &current.output_format, PathBuf::from(path))?
                .path
        } else {
            cache.offline_artifact_path(job_id, &current.output_format)?.path
        };
        self.patch_job(job_id, |job| {
            if let Some(artifact_url) = request.artifact_url {
                job.artifact_url = Some(artifact_url);
            }
            job.local_artifact_path = Some(artifact_path.to_string_lossy().into_owned());
            job.error_summary = None;
            job.status = OfflineJobStatus::Completed;
            job.stage = "completed".into();
            job.progress = 100;
            job.updated_at = Utc::now();
        })
    }

    pub fn fail_job(&self, job_id: &str, request: FailOfflineJobRequest) -> AppResult<OfflineJob> {
        let message = require_non_empty("message", request.message)?;
        self.fail_running_job(job_id, message)
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
        let mut jobs: Vec<_> = self
            .jobs
            .read()
            .expect("offline job manager lock poisoned")
            .values()
            .cloned()
            .collect();
        jobs.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        jobs
    }

    fn patch_job(&self, job_id: &str, patch: impl FnOnce(&mut OfflineJob)) -> AppResult<OfflineJob> {
        let mut jobs = self.jobs.write().expect("offline job manager lock poisoned");
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| AppError::offline_job(format!("offline job not found: {job_id}")))?;
        patch(job);
        let next = job.clone();
        self.persist_locked(&jobs)?;
        Ok(next)
    }

    fn patch_job_and_emit<F>(
        &self,
        job_id: &str,
        patch: impl FnOnce(&mut OfflineJob),
        on_update: &F,
    ) -> AppResult<OfflineJob>
    where
        F: Fn(OfflineJob),
    {
        let job = self.patch_job(job_id, patch)?;
        on_update(job.clone());
        Ok(job)
    }

    fn persist_locked(&self, jobs: &BTreeMap<String, OfflineJob>) -> AppResult<()> {
        if let Some(store) = &self.store {
            store.replace(OfflineJobStoreDoc {
                jobs: jobs.values().cloned().collect(),
            })?;
        }
        Ok(())
    }
}

fn prepare_audio_input(
    job_id: &str,
    input_ref: Option<String>,
    file_name: Option<String>,
    input_bytes: Option<Vec<u8>>,
    cache: &AssetCache,
) -> AppResult<(String, Option<String>)> {
    if let Some(bytes) = input_bytes {
        if bytes.is_empty() {
            return Err(AppError::offline_job("inputBytes is required for audio jobs"));
        }
        let file_name = require_non_empty("fileName", file_name.unwrap_or_else(|| "input.wav".into()))?;
        validate_input_format(&file_name)?;
        let path = cache.write_offline_input_bytes(job_id, &file_name, &bytes)?;
        return Ok((path.to_string_lossy().into_owned(), Some(file_name)));
    }

    let input_ref = require_non_empty("inputRef", input_ref.unwrap_or_default())?;
    validate_input_format(&input_ref)?;
    let input_file_name = Path::new(&input_ref)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);
    Ok((input_ref, input_file_name))
}

fn require_non_empty(field: &str, value: String) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::offline_job(format!("{field} is required")))
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_output_format(format: Option<String>, _fallback: &str) -> AppResult<String> {
    let value = format.unwrap_or_else(|| "wav".to_string());
    let normalized = value.trim().trim_start_matches('.').to_ascii_lowercase();
    match normalized.as_str() {
        "wav" => Ok(normalized),
        "mp3" => Err(AppError::offline_job("outputFormat currently only supports wav")),
        "" => Err(AppError::offline_job("outputFormat is required")),
        _ => Err(AppError::offline_job("outputFormat must be wav")),
    }
}

fn validate_input_format(value: &str) -> AppResult<()> {
    let format = Path::new(value)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or(value)
        .to_ascii_lowercase();
    if format == "wav" {
        Ok(())
    } else {
        Err(AppError::offline_job("audio input must be wav"))
    }
}

fn ensure_wav_audio(output_format: &str, audio_bytes: &[u8], content_type: Option<&str>) -> AppResult<()> {
    if output_format != "wav" || is_wav_bytes(audio_bytes) {
        return Ok(());
    }

    let content_type = content_type.unwrap_or("unknown");
    Err(AppError::offline_job(format!(
        "FunSpeech TTS returned non-WAV audio for wav output (content-type: {content_type})"
    )))
}

fn normalize_offline_wav_bytes(audio_bytes: &[u8]) -> AppResult<Vec<u8>> {
    normalize_wav_bytes(audio_bytes, AudioNormalizationConfig::default()).map(|(normalized, _report)| normalized)
}

fn is_wav_bytes(audio_bytes: &[u8]) -> bool {
    audio_bytes.len() >= 12 && &audio_bytes[0..4] == b"RIFF" && &audio_bytes[8..12] == b"WAVE"
}

fn default_submission_endpoint(endpoints: &OfflineEndpoints, input_type: &OfflineJobInputType) -> String {
    match input_type {
        OfflineJobInputType::Audio => endpoints.asr_url.clone(),
        OfflineJobInputType::Text => endpoints.tts_url.clone(),
    }
}

fn chunk_progress(index: usize, total_chunks: usize, phase: usize) -> u8 {
    let total_steps = total_chunks.saturating_mul(2).max(1);
    let step = index.saturating_mul(2).saturating_add(phase);
    (12 + (step.saturating_mul(74) / total_steps) as u8).min(86)
}

fn split_wav_file(path: &str, chunk_seconds: usize) -> AppResult<Vec<Vec<u8>>> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|error| AppError::offline_job(format!("failed to open wav input: {error}")))?;
    let source_spec = reader.spec();
    let channels = source_spec.channels.max(1) as usize;
    let sample_rate = source_spec.sample_rate as usize;
    if sample_rate == 0 {
        return Err(AppError::offline_job("wav input sample rate must be greater than 0"));
    }

    let samples = read_wav_samples(&mut reader, source_spec)?;
    if samples.is_empty() {
        return Err(AppError::offline_job("wav input contains no samples"));
    }

    let samples_per_chunk = sample_rate
        .saturating_mul(chunk_seconds.max(1))
        .saturating_mul(channels)
        .max(channels);
    samples
        .chunks(samples_per_chunk)
        .map(|chunk| write_wav_samples(chunk, channels as u16, source_spec.sample_rate))
        .collect()
}

fn wav_duration_seconds(path: &str) -> AppResult<f64> {
    let reader = hound::WavReader::open(path)
        .map_err(|error| AppError::offline_job(format!("failed to open wav input: {error}")))?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate as f64;
    if sample_rate <= 0.0 {
        return Err(AppError::offline_job("wav input sample rate must be greater than 0"));
    }
    Ok(reader.duration() as f64 / sample_rate)
}

fn remove_file_if_present(path: Option<&str>) -> AppResult<()> {
    if let Some(path) = path {
        let path = Path::new(path);
        if path.exists() {
            std::fs::remove_file(path).map_err(|source| AppError::io("removing stale offline artifact", source))?;
        }
    }
    Ok(())
}

fn merge_wav_chunks(chunks: &[Vec<u8>]) -> AppResult<Vec<u8>> {
    if chunks.is_empty() {
        return Err(AppError::offline_job("no wav chunks to merge"));
    }

    let mut merged_samples = Vec::new();
    let mut merged_channels = None;
    let mut merged_sample_rate = None;

    for chunk in chunks {
        let mut reader = hound::WavReader::new(Cursor::new(chunk.as_slice()))
            .map_err(|error| AppError::offline_job(format!("failed to open generated wav chunk: {error}")))?;
        let spec = reader.spec();
        let channels = spec.channels.max(1);
        let sample_rate = spec.sample_rate;
        if let Some(expected) = merged_channels {
            if expected != channels {
                return Err(AppError::offline_job(
                    "generated wav chunks have different channel counts",
                ));
            }
        } else {
            merged_channels = Some(channels);
        }
        if let Some(expected) = merged_sample_rate {
            if expected != sample_rate {
                return Err(AppError::offline_job(
                    "generated wav chunks have different sample rates",
                ));
            }
        } else {
            merged_sample_rate = Some(sample_rate);
        }

        merged_samples.extend(read_wav_samples(&mut reader, spec)?);
    }

    write_wav_samples(
        &merged_samples,
        merged_channels.unwrap_or(1),
        merged_sample_rate.unwrap_or(16_000),
    )
}

fn read_wav_samples<R: std::io::Read + std::io::Seek>(
    reader: &mut hound::WavReader<R>,
    spec: WavSpec,
) -> AppResult<Vec<f32>> {
    match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .map(|sample| sample.map(|value| value.clamp(-1.0, 1.0)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| AppError::offline_job(format!("failed to decode wav samples: {error}"))),
        SampleFormat::Int => {
            let max = (1_i64 << spec.bits_per_sample.saturating_sub(1) as u32) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| (value as f32 / max).clamp(-1.0, 1.0)))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| AppError::offline_job(format!("failed to decode wav samples: {error}")))
        }
    }
}

fn write_wav_samples(samples: &[f32], channels: u16, sample_rate: u32) -> AppResult<Vec<u8>> {
    let spec = WavSpec {
        channels: channels.max(1),
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|error| AppError::offline_job(format!("failed to create wav writer: {error}")))?;
        for sample in samples {
            writer
                .write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                .map_err(|error| AppError::offline_job(format!("failed to write wav sample: {error}")))?;
        }
        writer
            .finalize()
            .map_err(|error| AppError::offline_job(format!("failed to finalize wav chunk: {error}")))?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{
        domain::{offline_job::OfflineJobStatus, runtime_params::RuntimeParams, settings::AppSettings},
        services::asset_cache::AssetCache,
    };

    use super::{
        ensure_wav_audio, merge_wav_chunks, split_wav_file, wav_duration_seconds, CompleteOfflineJobRequest,
        CreateOfflineAudioJobRequest, CreateOfflineTextJobRequest, FailOfflineJobRequest, OfflineJobManager,
    };

    fn cache() -> AssetCache {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        AssetCache::new_with_inputs(
            std::env::temp_dir().join(format!("voice-cloner-offline-jobs-{unique}/offline")),
            std::env::temp_dir().join(format!("voice-cloner-offline-jobs-{unique}/inputs")),
            std::env::temp_dir().join(format!("voice-cloner-offline-jobs-{unique}/voice-design")),
        )
        .unwrap()
    }

    #[test]
    fn offline_manager_creates_audio_and_text_jobs_with_funspeech_endpoint_hints() {
        let manager = OfflineJobManager::default();
        let settings = AppSettings::default();
        let cache = cache();

        let audio = manager
            .create_audio_job(
                CreateOfflineAudioJobRequest {
                    input_ref: Some("C:/recordings/input.wav".into()),
                    file_name: None,
                    input_bytes: None,
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: Some("WAV".into()),
                },
                &settings,
                &cache,
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
        assert_eq!(audio.stage, "created");
        assert!(audio.artifact_url.unwrap().ends_with("/stream/v1/asr"));
        assert_eq!(audio.output_format, "wav");
        assert!(text.artifact_url.unwrap().ends_with("/stream/v1/tts"));
        assert_eq!(manager.list_jobs().len(), 2);
    }

    #[test]
    fn offline_manager_stores_uploaded_audio_inputs() {
        let manager = OfflineJobManager::default();
        let settings = AppSettings::default();
        let cache = cache();

        let audio = manager
            .create_audio_job(
                CreateOfflineAudioJobRequest {
                    input_ref: None,
                    file_name: Some("voice.WAV".into()),
                    input_bytes: Some(b"fake wav".to_vec()),
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: Some("wav".into()),
                },
                &settings,
                &cache,
            )
            .unwrap();

        assert_eq!(std::fs::read(audio.input_ref).unwrap(), b"fake wav");
        assert_eq!(audio.input_file_name.as_deref(), Some("voice.WAV"));
    }

    #[test]
    fn wav_splitter_splits_by_duration_and_merge_preserves_order() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("voice-cloner-split-{unique}.wav"));
        write_test_wav(&path, &[0.1, 0.2, 0.3, 0.4, 0.5], 1, 2);

        let chunks = split_wav_file(path.to_str().unwrap(), 1).unwrap();
        let merged = merge_wav_chunks(&chunks).unwrap();

        assert_eq!(chunks.len(), 3);
        assert_eq!(wav_sample_count(&merged), 5);
    }

    #[test]
    fn offline_manager_persists_recent_jobs() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let store_path = std::env::temp_dir().join(format!("voice-cloner-offline-jobs-{unique}/jobs.json"));
        let settings = AppSettings::default();
        let cache = cache();
        let manager = OfflineJobManager::new(&store_path).unwrap();

        let created = manager
            .create_text_job(
                CreateOfflineTextJobRequest {
                    text: "hello".into(),
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: Some("wav".into()),
                },
                &settings,
            )
            .unwrap();
        manager
            .complete_job(
                &created.job_id,
                CompleteOfflineJobRequest {
                    artifact_url: None,
                    local_artifact_path: None,
                },
                &cache,
            )
            .unwrap();

        let reloaded = OfflineJobManager::new(&store_path).unwrap();
        let jobs = reloaded.list_jobs();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, OfflineJobStatus::Completed);
        assert_eq!(jobs[0].progress, 100);
    }

    #[test]
    fn offline_manager_clears_records_and_audio_files() {
        let manager = OfflineJobManager::default();
        let settings = AppSettings::default();
        let cache = cache();
        let created = manager
            .create_text_job(
                CreateOfflineTextJobRequest {
                    text: "hello".into(),
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: Some("wav".into()),
                },
                &settings,
            )
            .unwrap();
        let artifact = cache
            .write_offline_artifact_bytes(&created.job_id, "wav", b"fake wav")
            .unwrap();
        manager
            .complete_job(
                &created.job_id,
                CompleteOfflineJobRequest {
                    artifact_url: None,
                    local_artifact_path: Some(artifact.path.to_string_lossy().into_owned()),
                },
                &cache,
            )
            .unwrap();

        let removed = manager.clear_jobs(&cache).unwrap();

        assert_eq!(removed.len(), 1);
        assert!(manager.list_jobs().is_empty());
        assert!(!artifact.path.exists());
    }

    #[test]
    fn offline_manager_deletes_one_record_and_audio_files() {
        let manager = OfflineJobManager::default();
        let settings = AppSettings::default();
        let cache = cache();
        let audio = manager
            .create_audio_job(
                CreateOfflineAudioJobRequest {
                    input_ref: None,
                    file_name: Some("input.wav".into()),
                    input_bytes: Some(b"fake input wav".to_vec()),
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: Some("wav".into()),
                },
                &settings,
                &cache,
            )
            .unwrap();
        let kept = manager
            .create_text_job(
                CreateOfflineTextJobRequest {
                    text: "keep me".into(),
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: Some("wav".into()),
                },
                &settings,
            )
            .unwrap();
        let completed = manager
            .complete_job(
                &audio.job_id,
                CompleteOfflineJobRequest {
                    artifact_url: None,
                    local_artifact_path: None,
                },
                &cache,
            )
            .unwrap();
        let input_path = completed.input_ref.clone();
        let artifact_path = completed.local_artifact_path.clone().unwrap();

        let removed = manager.delete_job(&audio.job_id, &cache).unwrap();

        assert_eq!(removed.job_id, audio.job_id);
        assert!(manager.get_job(&audio.job_id).is_err());
        assert!(manager.get_job(&kept.job_id).is_ok());
        assert!(!std::path::PathBuf::from(input_path).exists());
        assert!(!std::path::PathBuf::from(artifact_path).exists());
    }

    #[test]
    fn offline_manager_runs_fails_retries_and_completes_jobs() {
        let manager = OfflineJobManager::default();
        let settings = AppSettings::default();
        let cache = cache();
        let created = manager
            .create_audio_job(
                CreateOfflineAudioJobRequest {
                    input_ref: Some("C:/recordings/input.wav".into()),
                    file_name: None,
                    input_bytes: None,
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: Some("wav".into()),
                },
                &settings,
                &cache,
            )
            .unwrap();

        let failed = manager
            .fail_job(
                &created.job_id,
                FailOfflineJobRequest {
                    message: "backend failed".into(),
                },
            )
            .unwrap();
        assert_eq!(failed.status, OfflineJobStatus::Failed);
        assert_eq!(failed.stage, "failed");
        assert_eq!(failed.error_summary.as_deref(), Some("backend failed"));

        let retried = manager.retry_job(&created.job_id, &cache).unwrap();
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
        assert_eq!(completed.progress, 100);
        assert!(completed.local_artifact_path.unwrap().ends_with(".wav"));
    }

    #[test]
    fn retry_completed_job_reuses_record_and_removes_previous_artifact() {
        let manager = OfflineJobManager::default();
        let settings = AppSettings::default();
        let cache = cache();
        let created = manager
            .create_text_job(
                CreateOfflineTextJobRequest {
                    text: "hello".into(),
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: Some("wav".into()),
                },
                &settings,
            )
            .unwrap();
        let completed = manager
            .complete_with_audio_bytes(&created, &cache, &test_wav_bytes(&[0.2]), &|_| {})
            .unwrap();
        let old_path = completed.local_artifact_path.clone().unwrap();

        let retried = manager.retry_job(&created.job_id, &cache).unwrap();

        assert_eq!(retried.job_id, created.job_id);
        assert_eq!(retried.status, OfflineJobStatus::Created);
        assert_eq!(manager.list_jobs().len(), 1);
        assert!(!std::path::PathBuf::from(old_path).exists());
    }

    #[test]
    fn offline_manager_copies_completed_artifact_to_selected_path() {
        let manager = OfflineJobManager::default();
        let settings = AppSettings::default();
        let cache = cache();
        let created = manager
            .create_text_job(
                CreateOfflineTextJobRequest {
                    text: "hello".into(),
                    voice_name: "narrator".into(),
                    runtime_params: RuntimeParams::default(),
                    output_format: Some("wav".into()),
                },
                &settings,
            )
            .unwrap();
        manager
            .complete_with_audio_bytes(&created, &cache, &test_wav_bytes(&[0.2]), &|_| {})
            .unwrap();
        let target = std::env::temp_dir().join(format!("voice-cloner-download-{}.wav", created.job_id));

        let copied = manager.copy_artifact_to(&created.job_id, &target).unwrap();

        assert_eq!(copied, target);
        assert!(wav_peak(&std::fs::read(copied).unwrap()) > 0.88);
    }

    #[test]
    fn wav_duration_seconds_uses_sample_rate_and_channels() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("voice-cloner-duration-{unique}.wav"));
        write_test_wav(&path, &[0.1, 0.2, 0.3, 0.4], 2, 2);

        let duration = wav_duration_seconds(path.to_str().unwrap()).unwrap();

        assert!((duration - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ensure_wav_audio_rejects_non_wav_tts_bytes() {
        let error = ensure_wav_audio("wav", b"not-a-riff-file", Some("audio/mpeg")).unwrap_err();

        assert!(error.to_string().contains("non-WAV audio"));
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

    fn write_test_wav(path: &std::path::Path, samples: &[f32], channels: u16, sample_rate: u32) {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for sample in samples {
            writer.write_sample((sample * i16::MAX as f32) as i16).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn test_wav_bytes(samples: &[f32]) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
            for sample in samples {
                writer.write_sample((sample * i16::MAX as f32) as i16).unwrap();
            }
            writer.finalize().unwrap();
        }
        cursor.into_inner()
    }

    fn wav_sample_count(bytes: &[u8]) -> usize {
        let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        reader.samples::<i16>().count()
    }

    fn wav_peak(bytes: &[u8]) -> f32 {
        let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        reader
            .samples::<i16>()
            .map(|sample| (sample.unwrap() as f32 / i16::MAX as f32).abs())
            .fold(0.0_f32, f32::max)
    }
}
