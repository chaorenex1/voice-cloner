use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    app::error::{AppError, AppResult},
    domain::voice_separation::{AudioChannelMode, DenoiseMode, VoicePostProcessConfig},
};

use super::{command_available, command_version, write_log, SidecarPaths};

#[derive(Debug, Clone)]
pub struct FfmpegSidecar {
    binary_path: PathBuf,
}

impl Default for FfmpegSidecar {
    fn default() -> Self {
        Self::new(SidecarPaths::default().ffmpeg_path())
    }
}

impl FfmpegSidecar {
    pub fn new(binary_path: impl Into<PathBuf>) -> Self {
        Self {
            binary_path: binary_path.into(),
        }
    }

    pub fn available(&self) -> bool {
        command_available(&self.binary_path, "-version")
    }

    pub fn version(&self) -> Option<String> {
        command_version(&self.binary_path, "-version")
    }

    pub fn extract_audio_from_video(&self, input: &Path, output: &Path, log_path: &Path) -> AppResult<()> {
        self.run(
            vec![
                "-y".into(),
                "-i".into(),
                input.to_string_lossy().into_owned(),
                "-vn".into(),
                "-ac".into(),
                "2".into(),
                "-ar".into(),
                "44100".into(),
                "-c:a".into(),
                "pcm_s16le".into(),
                output.to_string_lossy().into_owned(),
            ],
            log_path,
            "extracting audio from video",
        )
    }

    pub fn transcode_audio_to_wav(&self, input: &Path, output: &Path, log_path: &Path) -> AppResult<()> {
        self.run(
            vec![
                "-y".into(),
                "-i".into(),
                input.to_string_lossy().into_owned(),
                "-ac".into(),
                "2".into(),
                "-ar".into(),
                "44100".into(),
                "-c:a".into(),
                "pcm_s16le".into(),
                output.to_string_lossy().into_owned(),
            ],
            log_path,
            "transcoding audio to wav",
        )
    }

    pub fn mix_no_vocals(
        &self,
        drums: &Path,
        bass: &Path,
        other: &Path,
        output: &Path,
        log_path: &Path,
    ) -> AppResult<()> {
        self.run(
            vec![
                "-y".into(),
                "-i".into(),
                drums.to_string_lossy().into_owned(),
                "-i".into(),
                bass.to_string_lossy().into_owned(),
                "-i".into(),
                other.to_string_lossy().into_owned(),
                "-filter_complex".into(),
                "amix=inputs=3:duration=longest:normalize=0".into(),
                "-c:a".into(),
                "pcm_s16le".into(),
                output.to_string_lossy().into_owned(),
            ],
            log_path,
            "mixing no-vocals stem",
        )
    }

    pub fn post_process_vocals(
        &self,
        input: &Path,
        output: &Path,
        config: &VoicePostProcessConfig,
        log_path: &Path,
    ) -> AppResult<()> {
        self.run(
            vec![
                "-y".into(),
                "-i".into(),
                input.to_string_lossy().into_owned(),
                "-af".into(),
                post_process_filter(config),
                "-ac".into(),
                channel_count(&config.channels).to_string(),
                "-ar".into(),
                config.target_sample_rate.to_string(),
                "-c:a".into(),
                "pcm_s16le".into(),
                output.to_string_lossy().into_owned(),
            ],
            log_path,
            "post-processing separated vocals",
        )
    }

    fn run(&self, args: Vec<String>, log_path: &Path, context: &'static str) -> AppResult<()> {
        let output = Command::new(&self.binary_path)
            .args(&args)
            .output()
            .map_err(|source| AppError::io("starting ffmpeg sidecar", source))?;
        let mut log = Vec::new();
        log.extend_from_slice(
            format!("command: {}\nargs: {}\n", self.binary_path.display(), args.join(" ")).as_bytes(),
        );
        log.extend_from_slice(&output.stdout);
        if !output.stderr.is_empty() {
            log.extend_from_slice(b"\n--- stderr ---\n");
            log.extend_from_slice(&output.stderr);
        }
        write_log(log_path, &log)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(AppError::offline_job(format!(
                "ffmpeg failed while {context} (exit: {:?})",
                output.status.code()
            )))
        }
    }
}

fn post_process_filter(config: &VoicePostProcessConfig) -> String {
    let mut filters = Vec::new();
    match config.denoise_mode {
        DenoiseMode::Off => {}
        DenoiseMode::Standard => filters.push("afftdn=nr=10:nf=-50".to_string()),
        DenoiseMode::Strong => filters.push("afftdn=nr=20:nf=-50".to_string()),
    }
    if config.trim_silence {
        filters.push("silenceremove=start_periods=1:start_duration=0.2:start_threshold=-50dB".to_string());
    }
    if config.loudness_normalization {
        filters.push(format!(
            "loudnorm=I={}:TP={}:LRA=11",
            config.target_lufs, config.true_peak_db
        ));
    }
    filters.push(format!("aresample={}", config.target_sample_rate));
    if config.channels == AudioChannelMode::Mono {
        filters.push("pan=mono|c0=0.5*c0+0.5*c1".to_string());
    }
    if config.peak_limiter {
        filters.push("alimiter=limit=0.95".to_string());
    }
    filters.join(",")
}

fn channel_count(channels: &AudioChannelMode) -> u16 {
    match channels {
        AudioChannelMode::Mono => 1,
        AudioChannelMode::Stereo => 2,
    }
}
