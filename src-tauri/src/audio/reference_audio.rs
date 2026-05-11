use std::{io::Cursor, path::Path};

use hound::{SampleFormat, WavSpec};

use crate::app::error::{AppError, AppResult};

const MAX_REFERENCE_SECONDS: f64 = 10.0;
const FADE_OUT_SECONDS: f64 = 0.25;

#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceAudioPrepareReport {
    pub truncated: bool,
    pub input_duration_seconds: f64,
    pub output_duration_seconds: f64,
}

pub fn prepare_voice_reference_wav_file_in_place(path: &Path) -> AppResult<ReferenceAudioPrepareReport> {
    let bytes = std::fs::read(path).map_err(|source| AppError::io("reading voice reference wav", source))?;
    let (prepared, report) = prepare_voice_reference_wav_bytes(&bytes)?;
    if report.truncated {
        std::fs::write(path, prepared).map_err(|source| AppError::io("writing voice reference wav", source))?;
    }
    Ok(report)
}

pub fn prepare_voice_reference_wav_bytes(audio_bytes: &[u8]) -> AppResult<(Vec<u8>, ReferenceAudioPrepareReport)> {
    let wav = decode_wav(audio_bytes)?;
    if wav.samples.is_empty() {
        return Err(AppError::audio("reference wav contains no samples"));
    }

    let channels = wav.channels.max(1) as usize;
    let input_frames = wav.samples.len() / channels;
    let max_frames = (MAX_REFERENCE_SECONDS * f64::from(wav.sample_rate)).round() as usize;
    let input_duration_seconds = input_frames as f64 / f64::from(wav.sample_rate);

    if input_frames <= max_frames {
        return Ok((
            audio_bytes.to_vec(),
            ReferenceAudioPrepareReport {
                truncated: false,
                input_duration_seconds,
                output_duration_seconds: input_duration_seconds,
            },
        ));
    }

    let output_len = max_frames.saturating_mul(channels);
    let mut samples = wav.samples[..output_len].to_vec();
    apply_tail_fade_out(&mut samples, channels, wav.sample_rate);
    let output = encode_wav(&samples, wav.channels, wav.sample_rate)?;
    Ok((
        output,
        ReferenceAudioPrepareReport {
            truncated: true,
            input_duration_seconds,
            output_duration_seconds: max_frames as f64 / f64::from(wav.sample_rate),
        },
    ))
}

#[derive(Debug)]
struct DecodedWav {
    samples: Vec<f32>,
    channels: u16,
    sample_rate: u32,
}

fn decode_wav(audio_bytes: &[u8]) -> AppResult<DecodedWav> {
    let cursor = Cursor::new(audio_bytes);
    let mut reader =
        hound::WavReader::new(cursor).map_err(|error| AppError::audio(format!("failed to open wav: {error}")))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1);
    let sample_rate = spec.sample_rate;
    if sample_rate == 0 {
        return Err(AppError::audio("wav sample rate must be greater than 0"));
    }
    let samples = match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .map(|sample| {
                sample
                    .map(|sample| sample.clamp(-1.0, 1.0))
                    .map_err(|error| AppError::audio(format!("failed to decode wav sample: {error}")))
            })
            .collect::<AppResult<Vec<_>>>()?,
        SampleFormat::Int => {
            let scale = max_int_amplitude(spec.bits_per_sample);
            reader
                .samples::<i32>()
                .map(|sample| {
                    sample
                        .map(|sample| (sample as f32 / scale).clamp(-1.0, 1.0))
                        .map_err(|error| AppError::audio(format!("failed to decode wav sample: {error}")))
                })
                .collect::<AppResult<Vec<_>>>()?
        }
    };
    Ok(DecodedWav {
        samples,
        channels,
        sample_rate,
    })
}

fn apply_tail_fade_out(samples: &mut [f32], channels: usize, sample_rate: u32) {
    let frame_count = samples.len() / channels.max(1);
    let fade_frames = ((FADE_OUT_SECONDS * f64::from(sample_rate)).round() as usize)
        .max(1)
        .min(frame_count);
    let fade_start = frame_count.saturating_sub(fade_frames);
    for frame_index in fade_start..frame_count {
        let remaining = frame_count.saturating_sub(frame_index + 1);
        let gain = remaining as f32 / fade_frames as f32;
        let sample_start = frame_index * channels;
        for channel in 0..channels {
            if let Some(sample) = samples.get_mut(sample_start + channel) {
                *sample *= gain;
            }
        }
    }
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
        let mut writer =
            hound::WavWriter::new(&mut cursor, spec).map_err(|error| AppError::audio(error.to_string()))?;
        for sample in samples {
            writer
                .write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                .map_err(|error| AppError::audio(error.to_string()))?;
        }
        writer.finalize().map_err(|error| AppError::audio(error.to_string()))?;
    }
    Ok(cursor.into_inner())
}

fn max_int_amplitude(bits_per_sample: u16) -> f32 {
    let bits = bits_per_sample.clamp(1, 32);
    (1u64 << (bits - 1)) as f32
}

#[cfg(test)]
mod tests {
    use super::prepare_voice_reference_wav_bytes;

    #[test]
    fn reference_audio_truncates_over_ten_seconds_and_fades_tail() {
        let input = wav_bytes(&vec![0.5; 11_000], 1, 1_000);

        let (output, report) = prepare_voice_reference_wav_bytes(&input).unwrap();
        let samples = wav_samples(&output);

        assert!(report.truncated);
        assert!((report.output_duration_seconds - 10.0).abs() < 0.001);
        assert_eq!(samples.len(), 10_000);
        assert_eq!(*samples.last().unwrap(), 0);
        assert!(samples[9_900].abs() < samples[9_000].abs());
    }

    #[test]
    fn reference_audio_leaves_short_wav_unchanged() {
        let input = wav_bytes(&vec![0.5; 9_000], 1, 1_000);

        let (output, report) = prepare_voice_reference_wav_bytes(&input).unwrap();

        assert!(!report.truncated);
        assert_eq!(output, input);
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

    fn wav_samples(bytes: &[u8]) -> Vec<i16> {
        let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        reader.samples::<i16>().map(Result::unwrap).collect()
    }
}
