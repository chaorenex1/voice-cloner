use std::{
    sync::{mpsc as std_mpsc, Arc, Mutex, RwLock},
    thread,
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    SampleFormat as CpalSampleFormat, StreamConfig,
};

use crate::{
    app::error::{AppError, AppResult},
    audio::{
        device_manager::AudioDeviceManager,
        frame::{AudioFrame, PcmFormat},
    },
};

pub trait VirtualMicAdapter: Send + Sync {
    fn is_available(&self) -> bool;
    fn start(&self, format: PcmFormat) -> AppResult<()>;
    fn write_frame(&self, frame: &AudioFrame) -> AppResult<()>;
    fn stop(&self) -> AppResult<()>;
}

#[derive(Debug, Default)]
pub struct SelectableVirtualMicAdapter {
    target_device_id: RwLock<Option<String>>,
    running_format: RwLock<Option<PcmFormat>>,
    accepted_frame_count: RwLock<u64>,
    output: RwLock<Option<VirtualMicOutputHandle>>,
}

impl SelectableVirtualMicAdapter {
    pub fn set_target_device_id(&self, target_device_id: Option<String>) {
        *self.target_device_id.write().expect("virtual mic lock poisoned") = target_device_id;
    }

    pub fn target_device_id(&self) -> Option<String> {
        self.target_device_id.read().expect("virtual mic lock poisoned").clone()
    }

    pub fn accepted_frame_count(&self) -> u64 {
        *self.accepted_frame_count.read().expect("virtual mic lock poisoned")
    }

    fn validate_target_device_id(&self) -> AppResult<String> {
        let Some(target_device_id) = self.target_device_id() else {
            return Err(AppError::audio(
                "virtualMicDeviceId must select a writable virtual microphone output device when virtual microphone is enabled",
            ));
        };

        let devices = AudioDeviceManager::default().list_output_devices()?;
        if devices.iter().any(|device| device.id == target_device_id) {
            Ok(target_device_id)
        } else {
            Err(AppError::audio(format!(
                "selected virtual microphone output device is unavailable: {target_device_id}"
            )))
        }
    }
}

impl VirtualMicAdapter for SelectableVirtualMicAdapter {
    fn is_available(&self) -> bool {
        self.validate_target_device_id().is_ok()
    }

    fn start(&self, format: PcmFormat) -> AppResult<()> {
        format.validate().map_err(AppError::audio)?;
        let target_device_id = self.validate_target_device_id()?;
        let device = AudioDeviceManager::default().output_device_by_id(Some(&target_device_id))?;
        let output = VirtualMicOutputHandle::start(device, format)?;

        if let Some(previous) = self.output.write().expect("virtual mic lock poisoned").take() {
            previous.stop();
        }
        *self.running_format.write().expect("virtual mic lock poisoned") = Some(format);
        *self.accepted_frame_count.write().expect("virtual mic lock poisoned") = 0;
        *self.output.write().expect("virtual mic lock poisoned") = Some(output);
        Ok(())
    }

    fn write_frame(&self, frame: &AudioFrame) -> AppResult<()> {
        let expected_format = *self.running_format.read().expect("virtual mic lock poisoned");
        let Some(expected_format) = expected_format else {
            return Err(AppError::audio("virtual microphone is not running"));
        };
        if frame.format != expected_format {
            return Err(AppError::audio("virtual microphone frame format changed while running"));
        }

        let output = self.output.read().expect("virtual mic lock poisoned");
        let Some(output) = output.as_ref() else {
            return Err(AppError::audio("virtual microphone output stream is not running"));
        };
        output.push_frame(frame)?;
        *self.accepted_frame_count.write().expect("virtual mic lock poisoned") += 1;
        Ok(())
    }

    fn stop(&self) -> AppResult<()> {
        if let Some(output) = self.output.write().expect("virtual mic lock poisoned").take() {
            output.stop();
        }
        *self.running_format.write().expect("virtual mic lock poisoned") = None;
        Ok(())
    }
}

#[derive(Debug)]
enum VirtualMicOutputCommand {
    Frame { samples: Vec<f32>, source_sample_rate: u32 },
    Stop,
}

#[derive(Debug)]
struct VirtualMicOutputHandle {
    tx: std_mpsc::Sender<VirtualMicOutputCommand>,
}

impl VirtualMicOutputHandle {
    fn start(device: cpal::Device, source_format: PcmFormat) -> AppResult<Self> {
        let (tx, rx) = std_mpsc::channel::<VirtualMicOutputCommand>();
        let (ready_tx, ready_rx) = std_mpsc::channel::<AppResult<()>>();
        thread::Builder::new()
            .name("virtual-mic-output".into())
            .spawn(move || {
                let mut output = match VirtualMicOutput::start(device, source_format) {
                    Ok(output) => {
                        let _ = ready_tx.send(Ok(()));
                        output
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };

                loop {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(VirtualMicOutputCommand::Frame {
                            samples,
                            source_sample_rate,
                        }) => {
                            if let Err(error) = output.push_samples(&samples, source_sample_rate) {
                                tracing::warn!(%error, "virtual microphone output write failed");
                            }
                        }
                        Ok(VirtualMicOutputCommand::Stop) | Err(std_mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(std_mpsc::RecvTimeoutError::Timeout) => output.suspend_if_idle(),
                    }
                }
            })
            .map_err(|source| AppError::io("starting virtual microphone output thread", source))?;
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| AppError::audio("virtual microphone output stream did not start"))??;
        Ok(Self { tx })
    }

    fn push_frame(&self, frame: &AudioFrame) -> AppResult<()> {
        self.tx
            .send(VirtualMicOutputCommand::Frame {
                samples: frame.samples.clone(),
                source_sample_rate: frame.format.sample_rate,
            })
            .map_err(|_| AppError::audio("virtual microphone output stream is closed"))
    }

    fn stop(self) {
        let _ = self.tx.send(VirtualMicOutputCommand::Stop);
    }
}

struct VirtualMicOutput {
    buffer: Arc<Mutex<VirtualMicSampleBuffer>>,
    output_sample_rate: u32,
    stream: cpal::Stream,
    playing: bool,
}

impl VirtualMicOutput {
    fn start(device: cpal::Device, source_format: PcmFormat) -> AppResult<Self> {
        let supported_config = device
            .default_output_config()
            .map_err(|error| AppError::audio(error.to_string()))?;
        let sample_format = supported_config.sample_format();
        let stream_config: StreamConfig = supported_config.into();
        let output_channels = stream_config.channels.max(1) as usize;
        let output_sample_rate = stream_config.sample_rate.0;
        let buffer = Arc::new(Mutex::new(VirtualMicSampleBuffer::default()));
        let stream = build_virtual_mic_output_stream(
            &device,
            &stream_config,
            sample_format,
            output_channels,
            Arc::clone(&buffer),
        )?;
        tracing::debug!(
            source_sample_rate = source_format.sample_rate,
            output_sample_rate,
            output_channels,
            "virtual microphone output initialized"
        );
        Ok(Self {
            buffer,
            output_sample_rate,
            stream,
            playing: false,
        })
    }

    fn push_samples(&mut self, samples: &[f32], source_sample_rate: u32) -> AppResult<()> {
        let samples = resample_mono_samples_linear(samples, source_sample_rate, self.output_sample_rate);
        if samples.is_empty() {
            return Ok(());
        }
        let max_buffered_samples = self.output_sample_rate as usize * 4;
        {
            let mut buffer = self.buffer.lock().expect("virtual mic buffer lock poisoned");
            buffer.extend_bounded(&samples, max_buffered_samples);
        }
        self.resume_if_needed()
    }

    fn resume_if_needed(&mut self) -> AppResult<()> {
        if self.playing {
            return Ok(());
        }
        self.stream.play().map_err(|error| AppError::audio(error.to_string()))?;
        self.playing = true;
        Ok(())
    }

    fn suspend_if_idle(&mut self) {
        if !self.playing || self.buffered_samples() > 0 {
            return;
        }
        match self.stream.pause() {
            Ok(()) => {
                self.playing = false;
            }
            Err(error) => {
                tracing::debug!(%error, "virtual microphone output pause not supported");
            }
        }
    }

    fn buffered_samples(&self) -> usize {
        self.buffer
            .lock()
            .expect("virtual mic buffer lock poisoned")
            .available()
    }
}

#[derive(Default)]
struct VirtualMicSampleBuffer {
    samples: Vec<f32>,
    cursor: usize,
}

impl VirtualMicSampleBuffer {
    fn extend_bounded(&mut self, samples: &[f32], max_samples: usize) {
        self.compact_if_needed();
        if samples.len() >= max_samples {
            self.samples.clear();
            self.cursor = 0;
            self.samples
                .extend_from_slice(&samples[samples.len().saturating_sub(max_samples)..]);
            return;
        }
        let overflow = self.available() + samples.len();
        if overflow > max_samples {
            self.cursor = (self.cursor + overflow - max_samples).min(self.samples.len());
            self.compact_if_needed();
        }
        self.samples.extend_from_slice(samples);
    }

    fn next_or_zero(&mut self) -> f32 {
        if self.cursor >= self.samples.len() {
            self.samples.clear();
            self.cursor = 0;
            return 0.0;
        }
        let sample = self.samples[self.cursor];
        self.cursor += 1;
        sample
    }

    fn available(&self) -> usize {
        self.samples.len().saturating_sub(self.cursor)
    }

    fn compact_if_needed(&mut self) {
        if self.cursor == 0 {
            return;
        }
        if self.cursor >= self.samples.len() {
            self.samples.clear();
            self.cursor = 0;
        } else if self.cursor > 4096 && self.cursor * 2 > self.samples.len() {
            self.samples.drain(..self.cursor);
            self.cursor = 0;
        }
    }
}

trait VirtualMicSample {
    fn from_f32(sample: f32) -> Self;
}

impl VirtualMicSample for f32 {
    fn from_f32(sample: f32) -> Self {
        sample.clamp(-1.0, 1.0)
    }
}

impl VirtualMicSample for i16 {
    fn from_f32(sample: f32) -> Self {
        (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
    }
}

impl VirtualMicSample for u16 {
    fn from_f32(sample: f32) -> Self {
        ((sample.clamp(-1.0, 1.0) + 1.0) * 0.5 * u16::MAX as f32) as u16
    }
}

fn build_virtual_mic_output_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    sample_format: CpalSampleFormat,
    output_channels: usize,
    buffer: Arc<Mutex<VirtualMicSampleBuffer>>,
) -> AppResult<cpal::Stream> {
    let err_fn = |error| tracing::warn!(%error, "virtual microphone output stream error");
    match sample_format {
        CpalSampleFormat::F32 => {
            let buffer = Arc::clone(&buffer);
            device
                .build_output_stream(
                    config,
                    move |data: &mut [f32], _| write_virtual_mic_output(data, output_channels, &buffer),
                    err_fn,
                    None,
                )
                .map_err(|error| AppError::audio(error.to_string()))
        }
        CpalSampleFormat::I16 => {
            let buffer = Arc::clone(&buffer);
            device
                .build_output_stream(
                    config,
                    move |data: &mut [i16], _| write_virtual_mic_output(data, output_channels, &buffer),
                    err_fn,
                    None,
                )
                .map_err(|error| AppError::audio(error.to_string()))
        }
        CpalSampleFormat::U16 => {
            let buffer = Arc::clone(&buffer);
            device
                .build_output_stream(
                    config,
                    move |data: &mut [u16], _| write_virtual_mic_output(data, output_channels, &buffer),
                    err_fn,
                    None,
                )
                .map_err(|error| AppError::audio(error.to_string()))
        }
        other => Err(AppError::audio(format!(
            "unsupported virtual microphone output sample format: {other:?}"
        ))),
    }
}

fn write_virtual_mic_output<T: VirtualMicSample>(
    output: &mut [T],
    output_channels: usize,
    buffer: &Arc<Mutex<VirtualMicSampleBuffer>>,
) {
    let mut samples = buffer.lock().expect("virtual mic buffer lock poisoned");
    for frame in output.chunks_mut(output_channels.max(1)) {
        let sample = samples.next_or_zero();
        for output_sample in frame {
            *output_sample = T::from_f32(sample);
        }
    }
}

fn resample_mono_samples_linear(samples: &[f32], source_sample_rate: u32, target_sample_rate: u32) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    let source_sample_rate = source_sample_rate.max(1);
    let target_sample_rate = target_sample_rate.max(1);
    if source_sample_rate == target_sample_rate {
        return samples.iter().map(|sample| sample.clamp(-1.0, 1.0)).collect();
    }
    let target_len = ((samples.len() as u64 * target_sample_rate as u64) / source_sample_rate as u64).max(1) as usize;
    (0..target_len)
        .map(|index| {
            let source_position = index as f64 * source_sample_rate as f64 / target_sample_rate as f64;
            let left = source_position.floor() as usize;
            let right = (left + 1).min(samples.len() - 1);
            let fraction = (source_position - left as f64) as f32;
            (samples[left] + (samples[right] - samples[left]) * fraction).clamp(-1.0, 1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{SelectableVirtualMicAdapter, VirtualMicAdapter};
    use crate::audio::frame::{AudioFrame, PcmFormat};

    #[test]
    fn selectable_virtual_mic_requires_selected_output_device() {
        let adapter = SelectableVirtualMicAdapter::default();

        let error = adapter.start(PcmFormat::default()).unwrap_err().to_string();

        assert!(error.contains("virtualMicDeviceId"));
    }

    #[test]
    fn selectable_virtual_mic_rejects_unknown_output_device() {
        let adapter = SelectableVirtualMicAdapter::default();
        adapter.set_target_device_id(Some("missing-output-device".into()));

        let error = adapter.start(PcmFormat::default()).unwrap_err().to_string();

        assert!(error.contains("unavailable"));
    }

    #[test]
    fn selectable_virtual_mic_rejects_frames_when_stopped() {
        let adapter = SelectableVirtualMicAdapter::default();
        let format = PcmFormat::default();
        let frame = AudioFrame {
            sequence: 1,
            timestamp_ms: Utc::now().timestamp_millis(),
            format,
            samples: vec![0.0; format.samples_per_frame()],
        };

        let error = adapter.write_frame(&frame).unwrap_err().to_string();

        assert!(error.contains("not running"));
    }

    #[test]
    #[ignore = "requires a local output audio device"]
    fn selectable_virtual_mic_local_smoke_writes_frame() {
        let output_devices = crate::audio::device_manager::AudioDeviceManager::default()
            .list_output_devices()
            .expect("local output devices can be listed");
        let adapter = SelectableVirtualMicAdapter::default();
        let format = PcmFormat::default();
        let frame = AudioFrame {
            sequence: 1,
            timestamp_ms: Utc::now().timestamp_millis(),
            format,
            samples: vec![0.0; format.samples_per_frame()],
        };

        for output_device in output_devices {
            adapter.set_target_device_id(Some(output_device.id));
            match adapter.start(format) {
                Ok(()) => {
                    adapter
                        .write_frame(&frame)
                        .expect("virtual mic adapter accepts a frame");
                    adapter.stop().expect("virtual mic adapter stops");

                    assert_eq!(adapter.accepted_frame_count(), 1);
                    return;
                }
                Err(error) => eprintln!(
                    "skipping unavailable virtual mic output candidate {}: {error}",
                    output_device.name
                ),
            }
        }

        eprintln!("skipping virtual mic smoke: no local output device accepted a test stream");
    }
}
