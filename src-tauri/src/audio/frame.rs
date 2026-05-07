use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SampleFormat {
    F32,
    I16,
    U16,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PcmFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: SampleFormat,
    pub frame_ms: u16,
}

impl PcmFormat {
    pub fn validate(&self) -> Result<(), String> {
        if self.sample_rate == 0 {
            return Err("sampleRate must be greater than 0".into());
        }
        if self.channels == 0 {
            return Err("channels must be greater than 0".into());
        }
        if self.frame_ms == 0 {
            return Err("frameMs must be greater than 0".into());
        }
        Ok(())
    }

    pub fn samples_per_frame(&self) -> usize {
        ((self.sample_rate as usize * self.frame_ms as usize) / 1000) * self.channels as usize
    }
}

impl Default for PcmFormat {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 1,
            sample_format: SampleFormat::F32,
            frame_ms: 20,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioFrame {
    pub sequence: u64,
    pub timestamp_ms: i64,
    pub format: PcmFormat,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioLevel {
    pub rms: f32,
    pub peak: f32,
}

pub fn measure_level(samples: &[f32]) -> AudioLevel {
    if samples.is_empty() {
        return AudioLevel { rms: 0.0, peak: 0.0 };
    }

    let mut sum = 0.0_f32;
    let mut peak = 0.0_f32;
    for sample in samples {
        let absolute = sample.abs();
        peak = peak.max(absolute);
        sum += sample * sample;
    }

    AudioLevel {
        rms: (sum / samples.len() as f32).sqrt(),
        peak,
    }
}

#[cfg(test)]
mod tests {
    use super::{measure_level, PcmFormat};

    #[test]
    fn pcm_format_calculates_frame_size() {
        let format = PcmFormat {
            sample_rate: 48_000,
            channels: 2,
            frame_ms: 20,
            ..Default::default()
        };

        assert_eq!(format.samples_per_frame(), 1_920);
        assert!(format.validate().is_ok());
    }

    #[test]
    fn measure_level_returns_peak_and_rms() {
        let level = measure_level(&[-1.0, 0.0, 1.0]);

        assert_eq!(level.peak, 1.0);
        assert!((level.rms - 0.816).abs() < 0.01);
    }
}
