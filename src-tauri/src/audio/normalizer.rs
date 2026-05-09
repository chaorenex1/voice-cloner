use std::{io::Cursor, path::Path};

use hound::{SampleFormat, WavSpec};

use crate::app::error::{AppError, AppResult};

const DB_EPSILON: f32 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioNormalizationConfig {
    pub enabled: bool,
    pub target_peak_dbfs: f32,
    pub max_gain_db: f32,
    pub silence_threshold: f32,
}

impl Default for AudioNormalizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            target_peak_dbfs: -1.0,
            max_gain_db: 18.0,
            silence_threshold: 0.00001,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioNormalizationReport {
    pub applied: bool,
    pub input_peak: f32,
    pub input_rms: f32,
    pub output_peak: f32,
    pub output_rms: f32,
    pub gain_db: f32,
    pub skipped_reason: Option<&'static str>,
}

pub fn normalize_wav_file_in_place(
    path: &Path,
    config: AudioNormalizationConfig,
) -> AppResult<AudioNormalizationReport> {
    let bytes = std::fs::read(path).map_err(|source| AppError::io("reading wav for normalization", source))?;
    let (normalized, report) = normalize_wav_bytes(&bytes, config)?;
    if report.applied {
        std::fs::write(path, normalized).map_err(|source| AppError::io("writing normalized wav", source))?;
    }
    Ok(report)
}

pub fn normalize_wav_bytes(
    audio_bytes: &[u8],
    config: AudioNormalizationConfig,
) -> AppResult<(Vec<u8>, AudioNormalizationReport)> {
    if !config.enabled {
        return Ok((
            audio_bytes.to_vec(),
            AudioNormalizationReport {
                applied: false,
                input_peak: 0.0,
                input_rms: 0.0,
                output_peak: 0.0,
                output_rms: 0.0,
                gain_db: 0.0,
                skipped_reason: Some("disabled"),
            },
        ));
    }

    let wav = decode_wav(audio_bytes)?;
    if wav.samples.is_empty() {
        return Err(AppError::audio("wav normalization requires at least one sample"));
    }

    let input = measure_samples(&wav.samples);
    if input.peak <= config.silence_threshold {
        return Ok((
            audio_bytes.to_vec(),
            AudioNormalizationReport {
                applied: false,
                input_peak: input.peak,
                input_rms: input.rms,
                output_peak: input.peak,
                output_rms: input.rms,
                gain_db: 0.0,
                skipped_reason: Some("silence"),
            },
        ));
    }

    let target_peak = db_to_linear(config.target_peak_dbfs);
    let required_gain = target_peak / input.peak;
    let required_gain_db = linear_to_db(required_gain);
    if required_gain_db.abs() <= DB_EPSILON {
        return Ok((
            audio_bytes.to_vec(),
            AudioNormalizationReport {
                applied: false,
                input_peak: input.peak,
                input_rms: input.rms,
                output_peak: input.peak,
                output_rms: input.rms,
                gain_db: 0.0,
                skipped_reason: Some("alreadyAtTarget"),
            },
        ));
    }

    let gain = if required_gain > 1.0 {
        required_gain.min(db_to_linear(config.max_gain_db.max(0.0)))
    } else {
        required_gain
    };
    let normalized_samples = wav
        .samples
        .iter()
        .map(|sample| (sample * gain).clamp(-1.0, 1.0))
        .collect::<Vec<_>>();
    let output = measure_samples(&normalized_samples);
    let normalized_bytes = encode_wav(&normalized_samples, wav.channels, wav.sample_rate)?;

    Ok((
        normalized_bytes,
        AudioNormalizationReport {
            applied: true,
            input_peak: input.peak,
            input_rms: input.rms,
            output_peak: output.peak,
            output_rms: output.rms,
            gain_db: linear_to_db(gain),
            skipped_reason: None,
        },
    ))
}

#[derive(Debug)]
struct WavBuffer {
    samples: Vec<f32>,
    channels: u16,
    sample_rate: u32,
}

#[derive(Debug, Clone, Copy)]
struct AudioStats {
    peak: f32,
    rms: f32,
}

fn decode_wav(audio_bytes: &[u8]) -> AppResult<WavBuffer> {
    let mut reader = hound::WavReader::new(Cursor::new(audio_bytes))
        .map_err(|error| AppError::audio(format!("failed to open wav for normalization: {error}")))?;
    let spec = reader.spec();
    let samples = match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .map(|sample| sample.map(|value| value.clamp(-1.0, 1.0)))
            .collect::<Result<Vec<_>, _>>(),
        SampleFormat::Int => {
            let max = (1_i64 << spec.bits_per_sample.saturating_sub(1) as u32) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| (value as f32 / max).clamp(-1.0, 1.0)))
                .collect::<Result<Vec<_>, _>>()
        }
    }
    .map_err(|error| AppError::audio(format!("failed to decode wav for normalization: {error}")))?;

    Ok(WavBuffer {
        samples,
        channels: spec.channels.max(1),
        sample_rate: spec.sample_rate,
    })
}

fn encode_wav(samples: &[f32], channels: u16, sample_rate: u32) -> AppResult<Vec<u8>> {
    let spec = WavSpec {
        channels: channels.max(1),
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|error| AppError::audio(format!("failed to create normalized wav: {error}")))?;
        for sample in samples {
            writer
                .write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                .map_err(|error| AppError::audio(format!("failed to write normalized wav: {error}")))?;
        }
        writer
            .finalize()
            .map_err(|error| AppError::audio(format!("failed to finalize normalized wav: {error}")))?;
    }
    Ok(cursor.into_inner())
}

fn measure_samples(samples: &[f32]) -> AudioStats {
    if samples.is_empty() {
        return AudioStats { peak: 0.0, rms: 0.0 };
    }
    let mut peak = 0.0_f32;
    let mut sum = 0.0_f32;
    for sample in samples {
        let absolute = sample.abs();
        peak = peak.max(absolute);
        sum += sample * sample;
    }
    AudioStats {
        peak,
        rms: (sum / samples.len() as f32).sqrt(),
    }
}

fn db_to_linear(db: f32) -> f32 {
    10_f32.powf(db / 20.0)
}

fn linear_to_db(value: f32) -> f32 {
    20.0 * value.max(f32::MIN_POSITIVE).log10()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{db_to_linear, normalize_wav_bytes, normalize_wav_file_in_place, AudioNormalizationConfig};

    #[test]
    fn normalizer_raises_quiet_wav_to_target_peak() {
        let input = wav_bytes(&[0.2, -0.05], 1, 16_000);

        let (normalized, report) = normalize_wav_bytes(&input, AudioNormalizationConfig::default()).unwrap();

        assert!(report.applied);
        assert!(report.gain_db > 12.0);
        assert_peak_close(&normalized, db_to_linear(-1.0));
    }

    #[test]
    fn normalizer_reduces_loud_wav_to_target_peak() {
        let input = wav_bytes(&[1.0, -0.5], 1, 16_000);

        let (normalized, report) = normalize_wav_bytes(&input, AudioNormalizationConfig::default()).unwrap();

        assert!(report.applied);
        assert!(report.gain_db < 0.0);
        assert_peak_close(&normalized, db_to_linear(-1.0));
    }

    #[test]
    fn normalizer_does_not_amplify_silence() {
        let input = wav_bytes(&[0.0, 0.0], 1, 16_000);

        let (normalized, report) = normalize_wav_bytes(&input, AudioNormalizationConfig::default()).unwrap();

        assert!(!report.applied);
        assert_eq!(report.skipped_reason, Some("silence"));
        assert_eq!(normalized, input);
    }

    #[test]
    fn normalizer_caps_max_gain() {
        let input = wav_bytes(&[0.001], 1, 16_000);
        let config = AudioNormalizationConfig {
            max_gain_db: 6.0,
            ..AudioNormalizationConfig::default()
        };

        let (normalized, report) = normalize_wav_bytes(&input, config).unwrap();

        assert!(report.applied);
        assert!((report.gain_db - 6.0).abs() < 0.1);
        assert!(wav_peak(&normalized) < 0.003);
    }

    #[test]
    fn normalizer_can_rewrite_file_in_place() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("voice-cloner-normalizer-{unique}.wav"));
        std::fs::write(&path, wav_bytes(&[0.2], 1, 16_000)).unwrap();

        let report = normalize_wav_file_in_place(&path, AudioNormalizationConfig::default()).unwrap();

        assert!(report.applied);
        assert_peak_close(&std::fs::read(path).unwrap(), db_to_linear(-1.0));
    }

    fn wav_bytes(samples: &[f32], channels: u16, sample_rate: u32) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
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

    fn assert_peak_close(bytes: &[u8], expected: f32) {
        let peak = wav_peak(bytes);
        assert!((peak - expected).abs() < 0.002, "peak {peak} != {expected}");
    }

    fn wav_peak(bytes: &[u8]) -> f32 {
        let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        reader
            .samples::<i16>()
            .map(|sample| (sample.unwrap() as f32 / i16::MAX as f32).abs())
            .fold(0.0_f32, f32::max)
    }
}
