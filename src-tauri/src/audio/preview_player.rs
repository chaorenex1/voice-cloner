use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    SampleFormat, StreamConfig,
};
use serde::Serialize;

use crate::app::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoicePreviewState {
    pub playing_voice_name: Option<String>,
}

#[derive(Debug, Default)]
pub struct VoicePreviewPlayer {
    current: Mutex<Option<PreviewHandle>>,
}

#[derive(Debug)]
struct PreviewHandle {
    voice_name: String,
    stop: Sender<()>,
}

#[derive(Debug)]
struct WavBuffer {
    samples: Arc<Vec<f32>>,
    channels: usize,
    sample_rate: u32,
}

impl VoicePreviewPlayer {
    pub fn toggle(
        &self,
        voice_name: String,
        wav_path: impl Into<PathBuf>,
        device: cpal::Device,
    ) -> AppResult<VoicePreviewState> {
        let wav_path = wav_path.into();
        let mut current = self.current.lock().expect("voice preview player lock poisoned");
        if current.as_ref().map(|handle| handle.voice_name.as_str()) == Some(voice_name.as_str()) {
            stop_current(&mut current);
            return Ok(VoicePreviewState {
                playing_voice_name: None,
            });
        }

        stop_current(&mut current);
        let stop = spawn_preview_thread(voice_name.clone(), wav_path, device)?;
        *current = Some(PreviewHandle { voice_name, stop });
        Ok(self.snapshot_from_guard(&current))
    }

    pub fn stop(&self) -> VoicePreviewState {
        let mut current = self.current.lock().expect("voice preview player lock poisoned");
        stop_current(&mut current);
        VoicePreviewState {
            playing_voice_name: None,
        }
    }

    pub fn snapshot(&self) -> VoicePreviewState {
        let current = self.current.lock().expect("voice preview player lock poisoned");
        self.snapshot_from_guard(&current)
    }

    fn snapshot_from_guard(&self, current: &Option<PreviewHandle>) -> VoicePreviewState {
        VoicePreviewState {
            playing_voice_name: current.as_ref().map(|handle| handle.voice_name.clone()),
        }
    }
}

fn stop_current(current: &mut Option<PreviewHandle>) {
    if let Some(handle) = current.take() {
        let _ = handle.stop.send(());
    }
}

fn spawn_preview_thread(voice_name: String, wav_path: PathBuf, device: cpal::Device) -> AppResult<Sender<()>> {
    let wav = load_wav(&wav_path)?;
    let supported_config = device
        .default_output_config()
        .map_err(|error| AppError::audio(error.to_string()))?;
    let sample_format = supported_config.sample_format();
    let stream_config: StreamConfig = supported_config.into();
    let output_channels = stream_config.channels as usize;
    let output_sample_rate = stream_config.sample_rate.0;
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<AppResult<()>>();

    thread::Builder::new()
        .name(format!("voice-preview-{voice_name}"))
        .spawn(move || {
            let stopped = Arc::new(AtomicBool::new(false));
            let cursor = Arc::new(AtomicUsize::new(0));
            let stream = match build_stream(
                &device,
                &stream_config,
                sample_format,
                wav,
                output_channels,
                output_sample_rate,
                Arc::clone(&cursor),
                Arc::clone(&stopped),
            ) {
                Ok(stream) => stream,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };
            if let Err(error) = stream.play() {
                let _ = ready_tx.send(Err(AppError::audio(error.to_string())));
                return;
            }
            let _ = ready_tx.send(Ok(()));
            while stop_rx.recv_timeout(Duration::from_millis(100)).is_err() {
                if stopped.load(Ordering::Relaxed) {
                    break;
                }
            }
            drop(stream);
        })
        .map_err(|source| AppError::io("starting voice preview thread", source))?;

    ready_rx
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| AppError::audio("voice preview player did not start"))??;
    Ok(stop_tx)
}

fn build_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
    wav: WavBuffer,
    output_channels: usize,
    output_sample_rate: u32,
    cursor: Arc<AtomicUsize>,
    stopped: Arc<AtomicBool>,
) -> AppResult<cpal::Stream> {
    let err_fn = |error| tracing::warn!(%error, "voice preview stream error");
    match sample_format {
        SampleFormat::F32 => device
            .build_output_stream(
                config,
                move |data: &mut [f32], _| {
                    write_output(data, &wav, output_channels, output_sample_rate, &cursor, &stopped)
                },
                err_fn,
                None,
            )
            .map_err(|error| AppError::audio(error.to_string())),
        SampleFormat::I16 => device
            .build_output_stream(
                config,
                move |data: &mut [i16], _| {
                    write_output(data, &wav, output_channels, output_sample_rate, &cursor, &stopped)
                },
                err_fn,
                None,
            )
            .map_err(|error| AppError::audio(error.to_string())),
        SampleFormat::U16 => device
            .build_output_stream(
                config,
                move |data: &mut [u16], _| {
                    write_output(data, &wav, output_channels, output_sample_rate, &cursor, &stopped)
                },
                err_fn,
                None,
            )
            .map_err(|error| AppError::audio(error.to_string())),
        other => Err(AppError::audio(format!("unsupported output sample format: {other:?}"))),
    }
}

trait PreviewSample {
    fn from_f32(sample: f32) -> Self;
}

impl PreviewSample for f32 {
    fn from_f32(sample: f32) -> Self {
        sample.clamp(-1.0, 1.0)
    }
}

impl PreviewSample for i16 {
    fn from_f32(sample: f32) -> Self {
        (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
    }
}

impl PreviewSample for u16 {
    fn from_f32(sample: f32) -> Self {
        ((sample.clamp(-1.0, 1.0) + 1.0) * 0.5 * u16::MAX as f32) as u16
    }
}

fn write_output<T: PreviewSample>(
    output: &mut [T],
    wav: &WavBuffer,
    output_channels: usize,
    output_sample_rate: u32,
    cursor: &AtomicUsize,
    stopped: &AtomicBool,
) {
    let frames = wav.samples.len() / wav.channels;
    for frame in output.chunks_mut(output_channels) {
        let output_frame = cursor.fetch_add(1, Ordering::Relaxed);
        let source_frame = output_frame * wav.sample_rate as usize / output_sample_rate as usize;
        if source_frame >= frames {
            stopped.store(true, Ordering::Relaxed);
            for sample in frame {
                *sample = T::from_f32(0.0);
            }
            continue;
        }
        for (channel, sample) in frame.iter_mut().enumerate() {
            let source_channel = channel.min(wav.channels - 1);
            let value = wav.samples[source_frame * wav.channels + source_channel];
            *sample = T::from_f32(value);
        }
    }
}

fn load_wav(path: &Path) -> AppResult<WavBuffer> {
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("wav"))
        != Some(true)
    {
        return Err(AppError::audio("voice preview only supports wav files"));
    }
    let mut reader = hound::WavReader::open(path)
        .map_err(|error| AppError::audio(format!("failed to open wav preview: {error}")))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let sample_rate = spec.sample_rate;
    let samples = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|sample| sample.map(|value| value.clamp(-1.0, 1.0)))
            .collect::<Result<Vec<_>, _>>(),
        hound::SampleFormat::Int => {
            let max = (1_i64 << (spec.bits_per_sample.saturating_sub(1) as u32)) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| (value as f32 / max).clamp(-1.0, 1.0)))
                .collect::<Result<Vec<_>, _>>()
        }
    }
    .map_err(|error| AppError::audio(format!("failed to decode wav preview: {error}")))?;
    if samples.is_empty() {
        return Err(AppError::audio("wav preview contains no samples"));
    }
    Ok(WavBuffer {
        samples: Arc::new(samples),
        channels,
        sample_rate,
    })
}
