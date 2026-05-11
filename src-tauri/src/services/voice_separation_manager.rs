use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::RwLock,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{
    app::{
        error::{AppError, AppResult},
        trace::{new_entity_id, TraceId},
    },
    audio::post_processor::AudioPostProcessor,
    domain::{
        voice::{CustomVoiceProfile, SyncStatus},
        voice_separation::{
            VoicePostProcessConfig, VoiceSeparationJob, VoiceSeparationModel, VoiceSeparationSourceType,
            VoiceSeparationStatus, VoiceSeparationStem, VoiceSeparationStems,
        },
    },
    services::{
        sidecar::{demucs_rs_sidecar::DemucsRsSidecar, ffmpeg_sidecar::FfmpegSidecar},
        voice_library::VoiceLibrary,
    },
};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreateVoiceSeparationJobRequest {
    pub source_path: String,
    pub model: Option<VoiceSeparationModel>,
    pub post_process_config: Option<VoicePostProcessConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SaveSeparatedVocalsRequest {
    pub voice_name: String,
    pub reference_text: String,
    pub voice_instruction: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSeparationMutationResult {
    pub job_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSeparationDownloadResult {
    pub target_path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSeparationRuntimeStatus {
    pub ffmpeg_available: bool,
    pub ffmpeg_version: Option<String>,
    pub demucs_rs_available: bool,
    pub demucs_rs_version: Option<String>,
    pub default_model_available: bool,
    pub model_cache_path: Option<String>,
    pub gpu_backend: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct VoiceSeparationManager {
    jobs_dir: PathBuf,
    jobs: RwLock<BTreeMap<String, VoiceSeparationJob>>,
    ffmpeg: FfmpegSidecar,
    demucs_rs: DemucsRsSidecar,
    post_processor: AudioPostProcessor,
}

impl VoiceSeparationManager {
    pub fn new(jobs_dir: impl Into<PathBuf>) -> AppResult<Self> {
        let jobs_dir = jobs_dir.into();
        std::fs::create_dir_all(&jobs_dir)
            .map_err(|source| AppError::io("creating voice separation jobs directory", source))?;
        let jobs = load_job_snapshots(&jobs_dir)?;
        Ok(Self {
            jobs_dir,
            jobs: RwLock::new(jobs),
            ffmpeg: FfmpegSidecar::default(),
            demucs_rs: DemucsRsSidecar::default(),
            post_processor: AudioPostProcessor::default(),
        })
    }

    pub fn runtime_status(&self) -> VoiceSeparationRuntimeStatus {
        let ffmpeg_available = self.ffmpeg.available();
        let demucs_rs_available = self.demucs_rs.available();
        let mut warnings = Vec::new();
        if !ffmpeg_available {
            warnings.push("ffmpeg sidecar is unavailable; video extraction and post-processing cannot run".into());
        }
        if !demucs_rs_available {
            warnings.push("demucs-rs sidecar is unavailable; local voice separation cannot run".into());
        }
        VoiceSeparationRuntimeStatus {
            ffmpeg_available,
            ffmpeg_version: self.ffmpeg.version(),
            demucs_rs_available,
            demucs_rs_version: self.demucs_rs.version(),
            default_model_available: false,
            model_cache_path: std::env::var_os("DEMUCS_RS_MODEL_CACHE")
                .map(|path| PathBuf::from(path).to_string_lossy().into_owned()),
            gpu_backend: None,
            warnings,
        }
    }

    pub fn create_job(&self, request: CreateVoiceSeparationJobRequest) -> AppResult<VoiceSeparationJob> {
        let source_path = require_existing_file(&request.source_path)?;
        let source_type = detect_source_type(&source_path)?;
        let now = Utc::now();
        let job_id = new_entity_id("voice-separation");
        let source_file_name = source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("source")
            .to_string();
        let job = VoiceSeparationJob {
            job_id: job_id.clone(),
            trace_id: TraceId::new("voice-separation").into_string(),
            source_type,
            source_path: source_path.to_string_lossy().into_owned(),
            source_file_name,
            model: request.model.unwrap_or(VoiceSeparationModel::HtDemucs),
            status: VoiceSeparationStatus::Queued,
            progress: 0.0,
            current_stage_message: "等待开始人声分离".into(),
            decoded_audio_path: None,
            stems: None,
            post_processed_vocals_path: None,
            post_process_report: None,
            reference_text: None,
            voice_name: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        };
        self.prepare_job_dirs(&job_id)?;
        self.write_job_snapshot(&job)?;
        self.jobs
            .write()
            .expect("voice separation jobs lock poisoned")
            .insert(job_id, job.clone());
        Ok(job)
    }

    pub fn start_job(
        &self,
        job_id: &str,
        post_process_config: Option<VoicePostProcessConfig>,
    ) -> AppResult<VoiceSeparationJob> {
        let job = self.update_job(job_id, |job| {
            if !matches!(
                job.status,
                VoiceSeparationStatus::Queued | VoiceSeparationStatus::Failed
            ) {
                return Err(AppError::offline_job(
                    "voice separation job is already running or completed",
                ));
            }
            job.error_message = None;
            job.transition_to(VoiceSeparationStatus::Decoding, 0.05, "准备源材料");
            Ok(job.clone())
        })?;

        let result = self.run_job(job.clone(), post_process_config.unwrap_or_default());
        match result {
            Ok(ready) => Ok(ready),
            Err(error) => {
                let message = error.to_string();
                let failed = self.update_job(job_id, |job| {
                    job.fail(message.clone());
                    Ok(job.clone())
                })?;
                Err(AppError::offline_job(format!(
                    "voice separation job failed: {}",
                    failed.error_message.clone().unwrap_or(message)
                )))
            }
        }
    }

    pub fn cancel_job(&self, job_id: &str) -> AppResult<VoiceSeparationJob> {
        self.update_job(job_id, |job| {
            job.transition_to(VoiceSeparationStatus::Cancelled, 1.0, "人声分离已取消");
            Ok(job.clone())
        })
    }

    pub fn delete_job(&self, job_id: &str) -> AppResult<VoiceSeparationMutationResult> {
        let job_dir = self.job_dir(job_id);
        self.jobs
            .write()
            .expect("voice separation jobs lock poisoned")
            .remove(job_id)
            .ok_or_else(|| AppError::offline_job(format!("voice separation job not found: {job_id}")))?;
        if job_dir.exists() {
            std::fs::remove_dir_all(&job_dir)
                .map_err(|source| AppError::io("deleting voice separation job directory", source))?;
        }
        Ok(VoiceSeparationMutationResult {
            job_id: job_id.to_string(),
            message: "人声分离任务已删除".into(),
        })
    }

    pub fn get_job(&self, job_id: &str) -> AppResult<VoiceSeparationJob> {
        self.jobs
            .read()
            .expect("voice separation jobs lock poisoned")
            .get(job_id)
            .cloned()
            .ok_or_else(|| AppError::offline_job(format!("voice separation job not found: {job_id}")))
    }

    pub fn list_jobs(&self) -> Vec<VoiceSeparationJob> {
        let mut jobs: Vec<_> = self
            .jobs
            .read()
            .expect("voice separation jobs lock poisoned")
            .values()
            .cloned()
            .collect();
        jobs.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.created_at.cmp(&a.created_at))
                .then_with(|| a.job_id.cmp(&b.job_id))
        });
        jobs
    }

    pub fn stem_path(&self, job_id: &str, stem: &VoiceSeparationStem) -> AppResult<PathBuf> {
        let job = self.get_job(job_id)?;
        if matches!(stem, VoiceSeparationStem::Vocals) {
            if let Some(path) = job.post_processed_vocals_path {
                return Ok(PathBuf::from(path));
            }
        }
        let stems = job
            .stems
            .as_ref()
            .ok_or_else(|| AppError::offline_job("voice separation job has no stems"))?;
        stems
            .path_for(stem)
            .map(PathBuf::from)
            .ok_or_else(|| AppError::offline_job("requested voice separation stem is unavailable"))
    }

    pub fn copy_stem_to(
        &self,
        job_id: &str,
        stem: &VoiceSeparationStem,
        target_path: impl Into<PathBuf>,
    ) -> AppResult<VoiceSeparationDownloadResult> {
        let source_path = self.stem_path(job_id, stem)?;
        let target_path = target_path.into();
        if target_path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("wav"))
            != Some(true)
        {
            return Err(AppError::offline_job(
                "voice separation stem download target must be a .wav file",
            ));
        }
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|source| AppError::io("creating voice separation download directory", source))?;
        }
        std::fs::copy(source_path, &target_path)
            .map_err(|source| AppError::io("copying voice separation stem", source))?;
        Ok(VoiceSeparationDownloadResult {
            target_path: target_path.to_string_lossy().into_owned(),
        })
    }

    pub fn processed_vocals_path(&self, job_id: &str) -> AppResult<PathBuf> {
        let job = self.get_job(job_id)?;
        job.post_processed_vocals_path
            .map(PathBuf::from)
            .ok_or_else(|| AppError::offline_job("voice separation job has no processed vocals"))
    }

    pub fn mark_reference_text(&self, job_id: &str, reference_text: String) -> AppResult<VoiceSeparationJob> {
        self.update_job(job_id, |job| {
            job.reference_text = Some(reference_text);
            job.updated_at = Utc::now();
            Ok(job.clone())
        })
    }

    pub fn save_as_custom_voice(
        &self,
        job_id: &str,
        request: SaveSeparatedVocalsRequest,
        library: &VoiceLibrary,
    ) -> AppResult<CustomVoiceProfile> {
        let voice_name = require_non_empty("voiceName", &request.voice_name)?;
        let reference_text = require_non_empty("referenceText", &request.reference_text)?;
        let vocals_path = self.processed_vocals_path(job_id)?;
        self.update_job(job_id, |job| {
            job.transition_to(VoiceSeparationStatus::SavingVoice, 0.95, "正在保存自定义音色");
            Ok(job.clone())
        })?;
        let profile = CustomVoiceProfile {
            voice_name: voice_name.clone(),
            source_prompt_text: Some("voiceSeparation".into()),
            asr_text: None,
            voice_instruction: request.voice_instruction.unwrap_or_default(),
            reference_audio_path: vocals_path.to_string_lossy().into_owned(),
            reference_text: reference_text.clone(),
            sync_status: SyncStatus::PendingSync,
            last_synced_at: None,
            created_at: Utc::now(),
        };
        let saved = library.save_custom_voice_preserving_audio(profile)?;
        self.update_job(job_id, |job| {
            job.voice_name = Some(saved.voice_name.clone());
            job.reference_text = Some(reference_text);
            job.transition_to(VoiceSeparationStatus::Saved, 1.0, "已保存为自定义音色");
            Ok(job.clone())
        })?;
        Ok(saved)
    }

    fn run_job(
        &self,
        mut job: VoiceSeparationJob,
        mut config: VoicePostProcessConfig,
    ) -> AppResult<VoiceSeparationJob> {
        config.trim_silence = false;
        let paths = JobPaths::new(self.job_dir(&job.job_id));
        self.prepare_job_dirs(&job.job_id)?;
        let source_path = PathBuf::from(&job.source_path);
        let separation_input = match job.source_type {
            VoiceSeparationSourceType::Video => {
                self.update_job(&job.job_id, |job| {
                    job.transition_to(VoiceSeparationStatus::ExtractingAudio, 0.1, "正在从视频提取音轨");
                    Ok(job.clone())
                })?;
                self.ffmpeg
                    .extract_audio_from_video(&source_path, &paths.decoded_audio, &paths.ffmpeg_decode_log)?;
                self.update_job(&job.job_id, |job| {
                    job.decoded_audio_path = Some(paths.decoded_audio.to_string_lossy().into_owned());
                    job.transition_to(VoiceSeparationStatus::Separating, 0.3, "正在本地分离人声");
                    Ok(job.clone())
                })?;
                paths.decoded_audio.clone()
            }
            VoiceSeparationSourceType::Audio => {
                self.update_job(&job.job_id, |job| {
                    job.transition_to(VoiceSeparationStatus::Separating, 0.2, "正在本地分离人声");
                    Ok(job.clone())
                })?;
                source_path
            }
        };

        let demucs_output = match self.demucs_rs.separate(
            &separation_input,
            &paths.raw_stems_dir,
            &job.model,
            &paths.demucs_stdout_log,
            &paths.demucs_stderr_log,
        ) {
            Ok(output) => output,
            Err(error) if matches!(job.source_type, VoiceSeparationSourceType::Audio) => {
                self.update_job(&job.job_id, |job| {
                    job.transition_to(
                        VoiceSeparationStatus::Decoding,
                        0.25,
                        "源音频解码失败，正在转码为 WAV 后重试",
                    );
                    Ok(job.clone())
                })?;
                self.ffmpeg.transcode_audio_to_wav(
                    &PathBuf::from(&job.source_path),
                    &paths.decoded_audio,
                    &paths.ffmpeg_decode_log,
                )?;
                self.update_job(&job.job_id, |job| {
                    job.decoded_audio_path = Some(paths.decoded_audio.to_string_lossy().into_owned());
                    job.transition_to(VoiceSeparationStatus::Separating, 0.35, "正在用转码后的 WAV 分离人声");
                    Ok(job.clone())
                })?;
                self.demucs_rs
                    .separate(
                        &paths.decoded_audio,
                        &paths.raw_stems_dir,
                        &job.model,
                        &paths.demucs_stdout_log,
                        &paths.demucs_stderr_log,
                    )
                    .map_err(|retry_error| {
                        AppError::offline_job(format!(
                            "demucs-rs failed after ffmpeg fallback: {retry_error}; first error: {error}"
                        ))
                    })?
            }
            Err(error) => return Err(error),
        };

        copy_to(&demucs_output.vocals, &paths.vocals_raw, "copying vocals stem")?;
        copy_to(&demucs_output.drums, &paths.drums_raw, "copying drums stem")?;
        copy_to(&demucs_output.bass, &paths.bass_raw, "copying bass stem")?;
        copy_to(&demucs_output.other, &paths.other_raw, "copying other stem")?;
        let stems = VoiceSeparationStems {
            vocals: Some(paths.vocals_raw.to_string_lossy().into_owned()),
            no_vocals: None,
            drums: Some(paths.drums_raw.to_string_lossy().into_owned()),
            bass: Some(paths.bass_raw.to_string_lossy().into_owned()),
            other: Some(paths.other_raw.to_string_lossy().into_owned()),
        };
        self.update_job(&job.job_id, |job| {
            job.stems = Some(stems.clone());
            job.transition_to(VoiceSeparationStatus::MixingNoVocals, 0.65, "正在合成伴奏音轨");
            Ok(job.clone())
        })?;

        self.ffmpeg.mix_no_vocals(
            &paths.drums_raw,
            &paths.bass_raw,
            &paths.other_raw,
            &paths.no_vocals,
            &paths.ffmpeg_mix_log,
        )?;
        self.update_job(&job.job_id, |job| {
            if let Some(stems) = job.stems.as_mut() {
                stems.no_vocals = Some(paths.no_vocals.to_string_lossy().into_owned());
            }
            job.transition_to(VoiceSeparationStatus::PostProcessing, 0.8, "正在后处理分离人声");
            Ok(job.clone())
        })?;

        let report = self.post_processor.process(
            &paths.vocals_raw,
            &paths.processed_vocals,
            &config,
            &paths.ffmpeg_post_process_log,
            &paths.post_process_report,
        )?;
        validate_wav_file(&paths.processed_vocals)?;
        job = self.update_job(&job.job_id, |job| {
            job.post_processed_vocals_path = Some(paths.processed_vocals.to_string_lossy().into_owned());
            job.post_process_report = Some(report.clone());
            job.transition_to(VoiceSeparationStatus::Ready, 1.0, "人声分离完成");
            Ok(job.clone())
        })?;
        Ok(job)
    }

    fn update_job<T>(
        &self,
        job_id: &str,
        change: impl FnOnce(&mut VoiceSeparationJob) -> AppResult<T>,
    ) -> AppResult<T> {
        let mut jobs = self.jobs.write().expect("voice separation jobs lock poisoned");
        let snapshot;
        let result = {
            let job = jobs
                .get_mut(job_id)
                .ok_or_else(|| AppError::offline_job(format!("voice separation job not found: {job_id}")))?;
            let result = change(job)?;
            snapshot = job.clone();
            result
        };
        drop(jobs);
        self.write_job_snapshot(&snapshot)?;
        Ok(result)
    }

    fn write_job_snapshot(&self, job: &VoiceSeparationJob) -> AppResult<()> {
        let path = self.job_dir(&job.job_id).join("job.json");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|source| AppError::io("creating voice separation job directory", source))?;
        }
        let bytes = serde_json::to_vec_pretty(job)
            .map_err(|source| AppError::json("serializing voice separation job", source))?;
        std::fs::write(path, bytes).map_err(|source| AppError::io("writing voice separation job", source))
    }

    fn prepare_job_dirs(&self, job_id: &str) -> AppResult<()> {
        let paths = JobPaths::new(self.job_dir(job_id));
        for dir in [
            &paths.root,
            &paths.source_dir,
            &paths.raw_stems_dir,
            &paths.stems_dir,
            &paths.processed_dir,
            &paths.reports_dir,
        ] {
            std::fs::create_dir_all(dir)
                .map_err(|source| AppError::io("creating voice separation job directory", source))?;
        }
        Ok(())
    }

    fn job_dir(&self, job_id: &str) -> PathBuf {
        self.jobs_dir.join(sanitize_path_segment(job_id))
    }
}

#[derive(Debug, Clone)]
struct JobPaths {
    root: PathBuf,
    source_dir: PathBuf,
    raw_stems_dir: PathBuf,
    stems_dir: PathBuf,
    processed_dir: PathBuf,
    reports_dir: PathBuf,
    decoded_audio: PathBuf,
    vocals_raw: PathBuf,
    drums_raw: PathBuf,
    bass_raw: PathBuf,
    other_raw: PathBuf,
    no_vocals: PathBuf,
    processed_vocals: PathBuf,
    ffmpeg_decode_log: PathBuf,
    demucs_stdout_log: PathBuf,
    demucs_stderr_log: PathBuf,
    ffmpeg_mix_log: PathBuf,
    ffmpeg_post_process_log: PathBuf,
    post_process_report: PathBuf,
}

impl JobPaths {
    fn new(root: PathBuf) -> Self {
        let source_dir = root.join("source");
        let raw_stems_dir = root.join("demucs-output");
        let stems_dir = root.join("stems");
        let processed_dir = root.join("processed");
        let reports_dir = root.join("reports");
        Self {
            decoded_audio: source_dir.join("input.decoded.wav"),
            vocals_raw: stems_dir.join("vocals.raw.wav"),
            drums_raw: stems_dir.join("drums.raw.wav"),
            bass_raw: stems_dir.join("bass.raw.wav"),
            other_raw: stems_dir.join("other.raw.wav"),
            no_vocals: stems_dir.join("no_vocals.wav"),
            processed_vocals: processed_dir.join("vocals.wav"),
            ffmpeg_decode_log: reports_dir.join("ffmpeg-decode.log"),
            demucs_stdout_log: reports_dir.join("demucs-rs.stdout.log"),
            demucs_stderr_log: reports_dir.join("demucs-rs.stderr.log"),
            ffmpeg_mix_log: reports_dir.join("ffmpeg-mix-no-vocals.log"),
            ffmpeg_post_process_log: reports_dir.join("ffmpeg-post-process.log"),
            post_process_report: reports_dir.join("post-process-report.json"),
            root,
            source_dir,
            raw_stems_dir,
            stems_dir,
            processed_dir,
            reports_dir,
        }
    }
}

fn require_existing_file(value: &str) -> AppResult<PathBuf> {
    let trimmed = require_non_empty("sourcePath", value)?;
    let path = PathBuf::from(trimmed);
    if !path.exists() || !path.is_file() {
        return Err(AppError::offline_job(format!(
            "source file not found: {}",
            path.display()
        )));
    }
    Ok(path)
}

fn detect_source_type(path: &Path) -> AppResult<VoiceSeparationSourceType> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ["mp4", "mov", "mkv", "webm", "avi"].contains(&ext.as_str()) {
        return Ok(VoiceSeparationSourceType::Video);
    }
    if ["wav", "mp3", "m4a", "aac", "flac", "ogg", "aiff", "aif"].contains(&ext.as_str()) {
        return Ok(VoiceSeparationSourceType::Audio);
    }
    Err(AppError::offline_job(format!(
        "unsupported source file extension: {ext}"
    )))
}

fn require_non_empty(field: &str, value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::offline_job(format!("{field} is required")))
    } else {
        Ok(trimmed.to_string())
    }
}

fn copy_to(source: &Path, target: &Path, context: &'static str) -> AppResult<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|source| AppError::io("creating voice separation artifact directory", source))?;
    }
    if source != target {
        std::fs::copy(source, target).map_err(|source| AppError::io(context, source))?;
    }
    Ok(())
}

fn validate_wav_file(path: &Path) -> AppResult<()> {
    let reader = hound::WavReader::open(path)
        .map_err(|error| AppError::audio(format!("failed to open processed vocals wav: {error}")))?;
    if reader.duration() == 0 {
        return Err(AppError::audio("processed vocals wav contains no samples"));
    }
    Ok(())
}

fn load_job_snapshots(jobs_dir: &Path) -> AppResult<BTreeMap<String, VoiceSeparationJob>> {
    let mut jobs = BTreeMap::new();
    for entry in
        std::fs::read_dir(jobs_dir).map_err(|source| AppError::io("reading voice separation jobs directory", source))?
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(source) => {
                warn!(error = %source, "skipping unreadable voice separation job directory entry");
                continue;
            }
        };
        let snapshot_path = entry.path().join("job.json");
        if !snapshot_path.is_file() {
            continue;
        }
        let bytes = match std::fs::read(&snapshot_path) {
            Ok(bytes) => bytes,
            Err(source) => {
                warn!(
                    path = %snapshot_path.display(),
                    error = %source,
                    "skipping unreadable voice separation job snapshot"
                );
                continue;
            }
        };
        let job: VoiceSeparationJob = match serde_json::from_slice(&bytes) {
            Ok(job) => job,
            Err(source) => {
                warn!(
                    path = %snapshot_path.display(),
                    error = %source,
                    "skipping invalid voice separation job snapshot"
                );
                continue;
            }
        };
        if job.job_id.trim().is_empty() {
            warn!(path = %snapshot_path.display(), "skipping voice separation job snapshot without job_id");
            continue;
        }
        jobs.insert(job.job_id.clone(), job);
    }
    Ok(jobs)
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
    use super::*;

    fn unique_temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "voice-cloner-{name}-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    #[test]
    fn voice_separation_manager_reloads_persisted_jobs_and_skips_corrupt_snapshots() {
        let root = unique_temp_dir("separation-history");
        let source_path = root.join("input.wav");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&source_path, b"placeholder").unwrap();

        let manager = VoiceSeparationManager::new(root.join("jobs")).unwrap();
        let older = manager
            .create_job(CreateVoiceSeparationJobRequest {
                source_path: source_path.to_string_lossy().into_owned(),
                model: None,
                post_process_config: None,
            })
            .unwrap();
        let newer = manager
            .create_job(CreateVoiceSeparationJobRequest {
                source_path: source_path.to_string_lossy().into_owned(),
                model: None,
                post_process_config: None,
            })
            .unwrap();
        manager
            .update_job(&older.job_id, |job| {
                job.transition_to(VoiceSeparationStatus::Ready, 1.0, "done later");
                Ok(())
            })
            .unwrap();

        let corrupt_dir = root.join("jobs").join("corrupt");
        std::fs::create_dir_all(&corrupt_dir).unwrap();
        std::fs::write(corrupt_dir.join("job.json"), b"{not-json").unwrap();

        let reloaded = VoiceSeparationManager::new(root.join("jobs")).unwrap();
        let jobs = reloaded.list_jobs();

        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].job_id, older.job_id);
        assert!(jobs.iter().any(|job| job.job_id == newer.job_id));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn voice_separation_vocals_stem_path_prefers_post_processed_audio() {
        let root = unique_temp_dir("processed-vocals-path");
        let source_path = root.join("input.wav");
        let processed_path = root.join("processed-vocals.wav");
        let raw_path = root.join("raw-vocals.wav");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&source_path, b"placeholder").unwrap();
        std::fs::write(&processed_path, b"processed").unwrap();
        std::fs::write(&raw_path, b"raw").unwrap();

        let manager = VoiceSeparationManager::new(root.join("jobs")).unwrap();
        let job = manager
            .create_job(CreateVoiceSeparationJobRequest {
                source_path: source_path.to_string_lossy().into_owned(),
                model: None,
                post_process_config: None,
            })
            .unwrap();
        manager
            .update_job(&job.job_id, |job| {
                job.stems = Some(VoiceSeparationStems {
                    vocals: Some(raw_path.to_string_lossy().into_owned()),
                    no_vocals: None,
                    drums: None,
                    bass: None,
                    other: None,
                });
                job.post_processed_vocals_path = Some(processed_path.to_string_lossy().into_owned());
                Ok(())
            })
            .unwrap();

        assert_eq!(
            manager.stem_path(&job.job_id, &VoiceSeparationStem::Vocals).unwrap(),
            processed_path
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
