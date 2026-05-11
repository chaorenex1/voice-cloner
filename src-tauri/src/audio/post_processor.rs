use std::path::{Path, PathBuf};

use hound::WavReader;

use crate::{
    app::error::{AppError, AppResult},
    domain::voice_separation::{AudioPostProcessReport, DenoiseMode, VoicePostProcessConfig},
    services::sidecar::ffmpeg_sidecar::FfmpegSidecar,
};

#[derive(Debug, Clone)]
pub struct AudioPostProcessor {
    ffmpeg: FfmpegSidecar,
}

impl Default for AudioPostProcessor {
    fn default() -> Self {
        Self {
            ffmpeg: FfmpegSidecar::default(),
        }
    }
}

impl AudioPostProcessor {
    pub fn new(ffmpeg: FfmpegSidecar) -> Self {
        Self { ffmpeg }
    }

    pub fn process(
        &self,
        input: &Path,
        output: &Path,
        config: &VoicePostProcessConfig,
        ffmpeg_log_path: &Path,
        report_path: &Path,
    ) -> AppResult<AudioPostProcessReport> {
        validate_config(config)?;
        let input_metrics = wav_metrics(input)?;
        self.ffmpeg.post_process_audio(input, output, config, ffmpeg_log_path)?;
        let output_metrics = wav_metrics(output)?;
        let mut warnings: Vec<String> = Vec::new();
        if output_metrics.sample_count == 0 {
            warnings.push("output contains no samples".into());
        }
        if output_metrics.peak <= 0.0001 {
            warnings.push("output is near silence".into());
        }
        if output_metrics.peak >= 0.999 {
            warnings.push("output peak is close to clipping".into());
        }
        if !config.trim_silence
            && duration_delta_exceeds_tolerance(input_metrics.duration_seconds, output_metrics.duration_seconds)
        {
            return Err(AppError::audio(format!(
                "processed vocals duration changed unexpectedly: input {:.3}s, output {:.3}s",
                input_metrics.duration_seconds, output_metrics.duration_seconds
            )));
        }
        if !warnings.is_empty()
            && warnings
                .iter()
                .any(|warning| warning.contains("no samples") || warning.contains("near silence"))
        {
            return Err(AppError::audio(format!(
                "invalid processed vocals: {}",
                warnings.join(", ")
            )));
        }

        let report = AudioPostProcessReport {
            input_duration_seconds: input_metrics.duration_seconds,
            output_duration_seconds: output_metrics.duration_seconds,
            input_sample_rate: input_metrics.sample_rate,
            output_sample_rate: output_metrics.sample_rate,
            input_channels: input_metrics.channels,
            output_channels: output_metrics.channels,
            denoise_applied: config.denoise_mode != DenoiseMode::Off,
            trim_applied: config.trim_silence,
            loudness_applied: config.loudness_normalization,
            peak_db: linear_to_db(output_metrics.peak),
            rms_db: linear_to_db(output_metrics.rms),
            warnings,
        };
        if let Some(parent) = report_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|source| AppError::io("creating post-process report directory", source))?;
        }
        let json = serde_json::to_vec_pretty(&report)
            .map_err(|source| AppError::json("serializing post-process report", source))?;
        std::fs::write(report_path, json).map_err(|source| AppError::io("writing post-process report", source))?;
        Ok(report)
    }

    pub fn process_wav_bytes(
        &self,
        audio_bytes: &[u8],
        config: &VoicePostProcessConfig,
        work_prefix: &str,
    ) -> AppResult<Vec<u8>> {
        let paths = TempPostProcessPaths::new(work_prefix);
        if let Some(parent) = paths.input.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|source| AppError::io("creating temporary post-process directory", source))?;
        }
        std::fs::write(&paths.input, audio_bytes)
            .map_err(|source| AppError::io("writing temporary post-process input", source))?;
        let result = self
            .process(&paths.input, &paths.output, config, &paths.log, &paths.report)
            .and_then(|_| {
                std::fs::read(&paths.output)
                    .map_err(|source| AppError::io("reading temporary post-process output", source))
            });
        paths.cleanup();
        result
    }
}

#[derive(Debug)]
struct TempPostProcessPaths {
    input: PathBuf,
    output: PathBuf,
    log: PathBuf,
    report: PathBuf,
}

impl TempPostProcessPaths {
    fn new(prefix: &str) -> Self {
        let safe_prefix = sanitize_temp_prefix(prefix);
        let unique = format!(
            "{}-{}-{}",
            safe_prefix,
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let root = std::env::temp_dir().join("voice-cloner-post-process").join(unique);
        Self {
            input: root.join("input.wav"),
            output: root.join("output.wav"),
            log: root.join("ffmpeg.log"),
            report: root.join("report.json"),
        }
    }

    fn cleanup(&self) {
        if let Some(root) = self.input.parent() {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

fn sanitize_temp_prefix(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect();
    if sanitized.is_empty() {
        "audio".into()
    } else {
        sanitized
    }
}

#[derive(Debug)]
struct WavMetrics {
    duration_seconds: f64,
    sample_rate: u32,
    channels: u16,
    sample_count: usize,
    peak: f32,
    rms: f32,
}

fn validate_config(config: &VoicePostProcessConfig) -> AppResult<()> {
    if !matches!(config.target_sample_rate, 16_000 | 24_000 | 44_100 | 48_000) {
        return Err(AppError::audio(
            "targetSampleRate must be 16000, 24000, 44100, or 48000",
        ));
    }
    if !(config.target_lufs >= -30.0 && config.target_lufs <= -8.0) {
        return Err(AppError::audio("targetLufs must be between -30 and -8"));
    }
    if !(config.true_peak_db >= -6.0 && config.true_peak_db <= 0.0) {
        return Err(AppError::audio("truePeakDb must be between -6 and 0"));
    }
    Ok(())
}

fn wav_metrics(path: &Path) -> AppResult<WavMetrics> {
    let mut reader = WavReader::open(path).map_err(|error| AppError::audio(format!("failed to open wav: {error}")))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1);
    let sample_rate = spec.sample_rate;
    if sample_rate == 0 {
        return Err(AppError::audio("wav sample rate must be greater than 0"));
    }
    let mut sample_count = 0usize;
    let mut peak = 0.0f32;
    let mut sum_squares = 0.0f64;
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for sample in reader.samples::<f32>() {
                let value = sample.map_err(|error| AppError::audio(format!("failed to decode wav sample: {error}")))?;
                let abs = value.abs();
                peak = peak.max(abs);
                sum_squares += f64::from(value * value);
                sample_count += 1;
            }
        }
        hound::SampleFormat::Int => {
            let scale = max_int_amplitude(spec.bits_per_sample);
            for sample in reader.samples::<i32>() {
                let value = sample.map_err(|error| AppError::audio(format!("failed to decode wav sample: {error}")))?
                    as f32
                    / scale;
                let abs = value.abs();
                peak = peak.max(abs);
                sum_squares += f64::from(value * value);
                sample_count += 1;
            }
        }
    }
    if sample_count == 0 {
        return Err(AppError::audio("wav contains no samples"));
    }
    let frames = sample_count as f64 / f64::from(channels);
    let rms = (sum_squares / sample_count as f64).sqrt() as f32;
    Ok(WavMetrics {
        duration_seconds: frames / f64::from(sample_rate),
        sample_rate,
        channels,
        sample_count,
        peak,
        rms,
    })
}

fn max_int_amplitude(bits_per_sample: u16) -> f32 {
    let bits = bits_per_sample.clamp(1, 32);
    (1u64 << (bits - 1)) as f32
}

fn linear_to_db(value: f32) -> f32 {
    if value <= 0.0 {
        -120.0
    } else {
        20.0 * value.log10()
    }
}

fn duration_delta_exceeds_tolerance(input_seconds: f64, output_seconds: f64) -> bool {
    let tolerance = (input_seconds * 0.005).max(0.05);
    (input_seconds - output_seconds).abs() > tolerance
}

#[cfg(test)]
mod tests {
    use super::duration_delta_exceeds_tolerance;

    #[test]
    fn duration_delta_allows_small_resampling_drift() {
        assert!(!duration_delta_exceeds_tolerance(30.0, 30.04));
        assert!(!duration_delta_exceeds_tolerance(300.0, 301.0));
    }

    #[test]
    fn duration_delta_rejects_truncated_processed_audio() {
        assert!(duration_delta_exceeds_tolerance(30.0, 25.0));
        assert!(duration_delta_exceeds_tolerance(3.0, 2.8));
    }
}
