use std::{
    collections::{BTreeMap, VecDeque},
    sync::{mpsc as std_mpsc, Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    SampleFormat as CpalSampleFormat, StreamConfig,
};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{
    app::error::{AppError, AppResult},
    audio::{
        frame::{AudioFrame, AudioLevel, PcmFormat},
        virtual_mic::{SelectableVirtualMicAdapter, VirtualMicAdapter},
    },
    domain::{runtime_params::RuntimeParams, session::RealtimeSession},
};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeStreamSnapshot {
    pub session_id: String,
    pub websocket_url: String,
    pub websocket_state: String,
    pub task_id: Option<String>,
    pub audio_mode: Option<String>,
    pub configured_voice_name: String,
    pub sent_frames: u64,
    pub received_frames: u64,
    pub sent_bytes: u64,
    pub received_bytes: u64,
    pub latency_ms: Option<u64>,
    pub input_level: AudioLevel,
    pub virtual_mic_frames: u64,
    pub last_event: Option<String>,
    pub last_error: Option<String>,
}

impl RealtimeStreamSnapshot {
    fn pending(session: &RealtimeSession) -> Self {
        Self {
            session_id: session.session_id.clone(),
            websocket_url: session.websocket_url.clone(),
            websocket_state: "connecting".into(),
            task_id: None,
            audio_mode: None,
            configured_voice_name: session.voice_name.clone(),
            sent_frames: 0,
            received_frames: 0,
            sent_bytes: 0,
            received_bytes: 0,
            latency_ms: None,
            input_level: AudioLevel { rms: 0.0, peak: 0.0 },
            virtual_mic_frames: 0,
            last_event: None,
            last_error: None,
        }
    }
}

#[derive(Debug)]
enum RealtimeControl {
    UpdateParams(RuntimeParams),
    SwitchVoice(String),
    Stop,
}

#[derive(Debug)]
struct RealtimeStreamHandle {
    control: mpsc::UnboundedSender<RealtimeControl>,
    snapshot: Arc<RwLock<RealtimeStreamSnapshot>>,
}

#[derive(Debug, Default)]
pub struct RealtimeStreamManager {
    streams: RwLock<BTreeMap<String, RealtimeStreamHandle>>,
}

impl RealtimeStreamManager {
    pub async fn start(
        &self,
        session: RealtimeSession,
        format: PcmFormat,
        input_device: Option<cpal::Device>,
        virtual_mic: Arc<SelectableVirtualMicAdapter>,
        write_virtual_mic: bool,
    ) -> AppResult<RealtimeStreamSnapshot> {
        tracing::debug!(
            session_id = %session.session_id,
            trace_id = %session.trace_id,
            voice_name = %session.voice_name,
            websocket_url = %session.websocket_url,
            sample_rate = format.sample_rate,
            frame_ms = format.frame_ms,
            write_virtual_mic,
            has_input_device = input_device.is_some(),
            "realtime stream start requested"
        );
        self.stop(&session.session_id).await;

        let snapshot = Arc::new(RwLock::new(RealtimeStreamSnapshot::pending(&session)));
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let session_id = session.session_id.clone();

        tokio::spawn(run_stream(
            session,
            format,
            input_device,
            virtual_mic,
            write_virtual_mic,
            Arc::clone(&snapshot),
            control_rx,
            ready_tx,
        ));

        match tokio::time::timeout(Duration::from_secs(8), ready_rx).await {
            Ok(Ok(Ok(()))) => {
                tracing::debug!(%session_id, "realtime stream ready");
                self.streams
                    .write()
                    .expect("realtime stream manager lock poisoned")
                    .insert(
                        session_id.clone(),
                        RealtimeStreamHandle {
                            control: control_tx,
                            snapshot: Arc::clone(&snapshot),
                        },
                    );
                Ok(snapshot.read().expect("realtime snapshot lock poisoned").clone())
            }
            Ok(Ok(Err(error))) => {
                tracing::warn!(%session_id, %error, "realtime stream failed while becoming ready");
                Err(error)
            }
            Ok(Err(_)) => {
                tracing::warn!(%session_id, "realtime stream exited before ready");
                Err(AppError::realtime_session(
                    "FunSpeech realtime stream exited before it became ready",
                ))
            }
            Err(_) => {
                tracing::warn!(%session_id, "realtime stream ready timeout");
                Err(AppError::realtime_session(
                    "timed out while connecting FunSpeech realtime stream",
                ))
            }
        }
    }

    pub async fn stop(&self, session_id: &str) -> Option<RealtimeStreamSnapshot> {
        tracing::debug!(%session_id, "realtime stream stop requested");
        let handle = self
            .streams
            .write()
            .expect("realtime stream manager lock poisoned")
            .remove(session_id)?;
        let _ = handle.control.send(RealtimeControl::Stop);
        let snapshot = handle.snapshot.read().expect("realtime snapshot lock poisoned").clone();
        tracing::debug!(
            %session_id,
            websocket_state = %snapshot.websocket_state,
            sent_frames = snapshot.sent_frames,
            received_frames = snapshot.received_frames,
            last_event = ?snapshot.last_event,
            last_error = ?snapshot.last_error,
            "realtime stream stop signal sent"
        );
        Some(snapshot)
    }

    pub fn update_params(&self, session_id: &str, runtime_params: RuntimeParams) -> AppResult<()> {
        self.send_control(session_id, RealtimeControl::UpdateParams(runtime_params))
    }

    pub fn switch_voice(&self, session_id: &str, voice_name: String) -> AppResult<()> {
        self.send_control(session_id, RealtimeControl::SwitchVoice(voice_name))
    }

    pub fn get_snapshot(&self, session_id: &str) -> AppResult<RealtimeStreamSnapshot> {
        let streams = self.streams.read().expect("realtime stream manager lock poisoned");
        streams
            .get(session_id)
            .map(|handle| handle.snapshot.read().expect("realtime snapshot lock poisoned").clone())
            .ok_or_else(|| AppError::realtime_session(format!("realtime stream not found: {session_id}")))
    }

    pub fn list_snapshots(&self) -> Vec<RealtimeStreamSnapshot> {
        self.streams
            .read()
            .expect("realtime stream manager lock poisoned")
            .values()
            .map(|handle| handle.snapshot.read().expect("realtime snapshot lock poisoned").clone())
            .collect()
    }

    fn send_control(&self, session_id: &str, control: RealtimeControl) -> AppResult<()> {
        tracing::debug!(%session_id, ?control, "sending realtime stream control");
        let streams = self.streams.read().expect("realtime stream manager lock poisoned");
        let handle = streams
            .get(session_id)
            .ok_or_else(|| AppError::realtime_session(format!("realtime stream not found: {session_id}")))?;
        handle
            .control
            .send(control)
            .map_err(|_| AppError::realtime_session("realtime stream control channel is closed"))
    }
}

async fn run_stream(
    session: RealtimeSession,
    format: PcmFormat,
    input_device: Option<cpal::Device>,
    virtual_mic: Arc<SelectableVirtualMicAdapter>,
    write_virtual_mic: bool,
    snapshot: Arc<RwLock<RealtimeStreamSnapshot>>,
    mut control_rx: mpsc::UnboundedReceiver<RealtimeControl>,
    ready_tx: oneshot::Sender<AppResult<()>>,
) {
    tracing::debug!(
        session_id = %session.session_id,
        trace_id = %session.trace_id,
        voice_name = %session.voice_name,
        websocket_url = %session.websocket_url,
        "realtime stream task starting"
    );
    let mut ready_tx = Some(ready_tx);
    let result = async {
        let (websocket, _) = connect_async(&session.websocket_url)
            .await
            .map_err(|error| AppError::realtime_session(format!("FunSpeech websocket connect failed: {error}")))?;
        tracing::debug!(
            session_id = %session.session_id,
            trace_id = %session.trace_id,
            "FunSpeech websocket connected"
        );
        let (mut write, mut read) = websocket.split();

        let started = read_json_event(&mut read).await?;
        tracing::debug!(
            session_id = %session.session_id,
            trace_id = %session.trace_id,
            event = ?event_name(&started),
            payload = %started,
            "FunSpeech realtime event received"
        );
        patch_snapshot(&snapshot, |state| apply_json_event(state, &started));
        if event_name(&started) != Some("session_started") {
            return Err(AppError::realtime_session(format!(
                "expected session_started from FunSpeech, got {started}"
            )));
        }

        let configure = json!({
            "event": "configure",
            "voice_name": session.voice_name,
            "format": "pcm",
            "sample_rate": format.sample_rate,
            "parameters": session.runtime_params.values,
        });
        write
            .send(Message::Text(configure.to_string()))
            .await
            .map_err(websocket_error)?;
        tracing::debug!(
            session_id = %session.session_id,
            trace_id = %session.trace_id,
            voice_name = %session.voice_name,
            sample_rate = format.sample_rate,
            frame_ms = format.frame_ms,
            "FunSpeech configure event sent"
        );

        let configured = read_json_event(&mut read).await?;
        tracing::debug!(
            session_id = %session.session_id,
            trace_id = %session.trace_id,
            event = ?event_name(&configured),
            payload = %configured,
            "FunSpeech realtime event received"
        );
        patch_snapshot(&snapshot, |state| apply_json_event(state, &configured));
        if event_name(&configured) != Some("configured") {
            return Err(AppError::realtime_session(format!(
                "expected configured from FunSpeech, got {configured}"
            )));
        }

        patch_snapshot(&snapshot, |state| {
            state.websocket_state = "running".into();
            state.last_error = None;
        });
        if let Some(tx) = ready_tx.take() {
            let _ = tx.send(Ok(()));
        }

        let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<RealtimeAudioChunk>();
        let _input_stop = if let Some(device) = input_device {
            tracing::debug!(
                session_id = %session.session_id,
                trace_id = %session.trace_id,
                "starting realtime input capture thread"
            );
            Some(spawn_input_thread(device, audio_tx.clone(), format)?)
        } else {
            tracing::debug!(
                session_id = %session.session_id,
                trace_id = %session.trace_id,
                "no input device resolved; using silence source"
            );
            spawn_silence_source(audio_tx.clone(), format);
            None
        };
        let frame_samples = vec![0.0; format.samples_per_frame()];
        let mut sequence = 0_u64;
        let mut pending_sends = VecDeque::<Instant>::new();

        loop {
            tokio::select! {
                Some(chunk) = audio_rx.recv() => {
                    write.send(Message::Binary(chunk.bytes.clone())).await.map_err(websocket_error)?;
                    sequence += 1;
                    pending_sends.push_back(Instant::now());
                    patch_snapshot(&snapshot, |state| {
                        state.sent_frames += 1;
                        state.sent_bytes += chunk.bytes.len() as u64;
                        state.input_level = chunk.level;
                    });
                    if sequence % 50 == 0 {
                        tracing::debug!(
                            session_id = %session.session_id,
                            trace_id = %session.trace_id,
                            sent_frames = sequence,
                            chunk_bytes = chunk.bytes.len(),
                            input_rms = chunk.level.rms,
                            input_peak = chunk.level.peak,
                            "realtime audio frames sent"
                        );
                    }
                }
                Some(control) = control_rx.recv() => {
                    match control {
                        RealtimeControl::UpdateParams(runtime_params) => {
                            let message = json!({
                                "event": "update",
                                "parameters": runtime_params.values,
                            });
                            write.send(Message::Text(message.to_string())).await.map_err(websocket_error)?;
                            tracing::debug!(
                                session_id = %session.session_id,
                                trace_id = %session.trace_id,
                                params = ?runtime_params.values,
                                "realtime params update sent"
                            );
                        }
                        RealtimeControl::SwitchVoice(voice_name) => {
                            let message = json!({
                                "event": "switch_voice",
                                "voice_name": voice_name,
                            });
                            write.send(Message::Text(message.to_string())).await.map_err(websocket_error)?;
                            tracing::debug!(
                                session_id = %session.session_id,
                                trace_id = %session.trace_id,
                                %voice_name,
                                "realtime voice switch sent"
                            );
                        }
                        RealtimeControl::Stop => {
                            let _ = write.send(Message::Text(json!({"event": "stop"}).to_string())).await;
                            tracing::debug!(
                                session_id = %session.session_id,
                                trace_id = %session.trace_id,
                                "realtime stop event sent"
                            );
                            patch_snapshot(&snapshot, |state| {
                                state.websocket_state = "stopping".into();
                                state.last_event = Some("stop_requested".into());
                            });
                            break;
                        }
                    }
                }
                message = read.next() => {
                    match message {
                        Some(Ok(Message::Binary(bytes))) => {
                            let latency_ms = pending_sends
                                .pop_front()
                                .map(|sent_at| sent_at.elapsed().as_millis().min(u64::MAX as u128) as u64);
                            let samples = pcm_i16_bytes_to_f32(&bytes, format.samples_per_frame()).unwrap_or_else(|| frame_samples.clone());
                            let frame = AudioFrame {
                                sequence,
                                timestamp_ms: chrono::Utc::now().timestamp_millis(),
                                format,
                                samples,
                            };
                            let virtual_mic_result = if write_virtual_mic {
                                virtual_mic.write_frame(&frame)
                            } else {
                                Ok(())
                            };
                            patch_snapshot(&snapshot, |state| {
                                state.received_frames += 1;
                                state.received_bytes += bytes.len() as u64;
                                state.latency_ms = latency_ms;
                                if write_virtual_mic && virtual_mic_result.is_ok() {
                                    state.virtual_mic_frames += 1;
                                } else if let Err(error) = &virtual_mic_result {
                                    state.last_error = Some(error.to_string());
                                }
                            });
                            if sequence % 50 == 0 {
                                tracing::debug!(
                                    session_id = %session.session_id,
                                    trace_id = %session.trace_id,
                                    received_for_sequence = sequence,
                                    response_bytes = bytes.len(),
                                    latency_ms = ?latency_ms,
                                    write_virtual_mic,
                                    virtual_mic_ok = virtual_mic_result.is_ok(),
                                    "realtime audio frame received"
                                );
                            }
                        }
                        Some(Ok(Message::Text(text))) => {
                            match serde_json::from_str::<Value>(&text) {
                                Ok(value) => {
                                    tracing::debug!(
                                        session_id = %session.session_id,
                                        trace_id = %session.trace_id,
                                        event = ?event_name(&value),
                                        payload = %value,
                                        "FunSpeech realtime event received"
                                    );
                                    patch_snapshot(&snapshot, |state| apply_json_event(state, &value));
                                    if event_name(&value) == Some("session_completed") {
                                        break;
                                    }
                                }
                                Err(error) => {
                                    tracing::warn!(
                                        session_id = %session.session_id,
                                        trace_id = %session.trace_id,
                                        %error,
                                        %text,
                                        "invalid FunSpeech event JSON"
                                    );
                                    patch_snapshot(&snapshot, |state| {
                                        state.last_error = Some(format!("invalid FunSpeech event JSON: {error}"));
                                    });
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            tracing::debug!(
                                session_id = %session.session_id,
                                trace_id = %session.trace_id,
                                "FunSpeech websocket closed"
                            );
                            patch_snapshot(&snapshot, |state| {
                                state.websocket_state = "closed".into();
                                state.last_event = Some("closed".into());
                            });
                            break;
                        }
                        Some(Ok(_)) => {}
                        Some(Err(error)) => return Err(websocket_error(error)),
                    }
                }
            }
        }

        patch_snapshot(&snapshot, |state| {
            if state.websocket_state != "closed" {
                state.websocket_state = "stopped".into();
            }
        });
        tracing::debug!(
            session_id = %session.session_id,
            trace_id = %session.trace_id,
            "realtime stream task stopped"
        );
        Ok(())
    }
    .await;

    if let Err(error) = result {
        tracing::warn!(
            session_id = %session.session_id,
            trace_id = %session.trace_id,
            %error,
            "realtime stream task failed"
        );
        patch_snapshot(&snapshot, |state| {
            state.websocket_state = "error".into();
            state.last_error = Some(error.to_string());
        });
        if let Some(tx) = ready_tx.take() {
            let _ = tx.send(Err(error));
        }
    }
}

async fn read_json_event<S>(read: &mut S) -> AppResult<Value>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    match tokio::time::timeout(Duration::from_secs(8), read.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => serde_json::from_str(&text)
            .map_err(|error| AppError::realtime_session(format!("invalid FunSpeech JSON event: {error}"))),
        Ok(Some(Ok(other))) => Err(AppError::realtime_session(format!(
            "expected FunSpeech JSON event, got {other:?}"
        ))),
        Ok(Some(Err(error))) => Err(websocket_error(error)),
        Ok(None) => Err(AppError::realtime_session(
            "FunSpeech websocket closed before sending ready event",
        )),
        Err(_) => Err(AppError::realtime_session(
            "timed out waiting for FunSpeech websocket event",
        )),
    }
}

fn silent_pcm_i16_frame(format: PcmFormat) -> Vec<u8> {
    vec![0; format.samples_per_frame() * std::mem::size_of::<i16>()]
}

#[derive(Debug)]
struct RealtimeAudioChunk {
    bytes: Vec<u8>,
    level: AudioLevel,
}

fn spawn_silence_source(tx: mpsc::UnboundedSender<RealtimeAudioChunk>, format: PcmFormat) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(format.frame_ms.max(1) as u64));
        let bytes = silent_pcm_i16_frame(format);
        loop {
            interval.tick().await;
            if tx
                .send(RealtimeAudioChunk {
                    bytes: bytes.clone(),
                    level: AudioLevel { rms: 0.0, peak: 0.0 },
                })
                .is_err()
            {
                break;
            }
        }
    });
}

fn spawn_input_thread(
    device: cpal::Device,
    tx: mpsc::UnboundedSender<RealtimeAudioChunk>,
    _format: PcmFormat,
) -> AppResult<std_mpsc::Sender<()>> {
    let (stop_tx, stop_rx) = std_mpsc::channel::<()>();
    let (ready_tx, ready_rx) = std_mpsc::channel::<AppResult<()>>();
    thread::Builder::new()
        .name("realtime-input-capture".into())
        .spawn(move || {
            let result = start_input_stream_on_thread(device, tx);
            match result {
                Ok(stream) => {
                    let _ = ready_tx.send(Ok(()));
                    loop {
                        match stop_rx.recv_timeout(Duration::from_millis(100)) {
                            Ok(()) | Err(std_mpsc::RecvTimeoutError::Disconnected) => break,
                            Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                        }
                    }
                    drop(stream);
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                }
            }
        })
        .map_err(|source| AppError::io("starting realtime input thread", source))?;
    ready_rx
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| AppError::audio("realtime input stream did not start"))??;
    Ok(stop_tx)
}

fn start_input_stream_on_thread(
    device: cpal::Device,
    tx: mpsc::UnboundedSender<RealtimeAudioChunk>,
) -> AppResult<cpal::Stream> {
    let supported_config = device
        .default_input_config()
        .map_err(|error| AppError::audio(error.to_string()))?;
    let sample_format = supported_config.sample_format();
    let config: StreamConfig = supported_config.into();
    let channels = config.channels.max(1) as usize;
    let err_fn = |error| tracing::warn!(%error, "realtime input stream error");

    let stream = match sample_format {
        CpalSampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _| send_input_chunk(data, channels, &tx),
            err_fn,
            None,
        ),
        CpalSampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _| send_input_chunk(data, channels, &tx),
            err_fn,
            None,
        ),
        CpalSampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _| send_input_chunk(data, channels, &tx),
            err_fn,
            None,
        ),
        other => return Err(AppError::audio(format!("unsupported input sample format: {other:?}"))),
    }
    .map_err(|error| AppError::audio(error.to_string()))?;

    stream.play().map_err(|error| AppError::audio(error.to_string()))?;
    Ok(stream)
}

trait RealtimeInputSample {
    fn to_f32(self) -> f32;
}

impl RealtimeInputSample for f32 {
    fn to_f32(self) -> f32 {
        self.clamp(-1.0, 1.0)
    }
}

impl RealtimeInputSample for i16 {
    fn to_f32(self) -> f32 {
        self as f32 / i16::MAX as f32
    }
}

impl RealtimeInputSample for u16 {
    fn to_f32(self) -> f32 {
        (self as f32 / u16::MAX as f32) * 2.0 - 1.0
    }
}

fn send_input_chunk<T: Copy + RealtimeInputSample>(
    data: &[T],
    channels: usize,
    tx: &mpsc::UnboundedSender<RealtimeAudioChunk>,
) {
    if data.is_empty() {
        return;
    }
    let mut mono = Vec::with_capacity(data.len() / channels.max(1));
    for frame in data.chunks(channels.max(1)) {
        let mixed = frame.iter().map(|sample| (*sample).to_f32()).sum::<f32>() / frame.len() as f32;
        mono.push(mixed.clamp(-1.0, 1.0));
    }
    let level = crate::audio::frame::measure_level(&mono);
    let mut bytes = Vec::with_capacity(mono.len() * std::mem::size_of::<i16>());
    for sample in mono {
        bytes.extend_from_slice(&((sample * i16::MAX as f32) as i16).to_le_bytes());
    }
    let _ = tx.send(RealtimeAudioChunk { bytes, level });
}

fn pcm_i16_bytes_to_f32(bytes: &[u8], min_samples: usize) -> Option<Vec<f32>> {
    if bytes.len() < 2 {
        return None;
    }
    let mut samples = bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / i16::MAX as f32)
        .collect::<Vec<_>>();
    if samples.len() < min_samples {
        samples.resize(min_samples, 0.0);
    }
    Some(samples)
}

fn event_name(value: &Value) -> Option<&str> {
    value.get("event").and_then(Value::as_str)
}

fn apply_json_event(state: &mut RealtimeStreamSnapshot, value: &Value) {
    if let Some(event) = event_name(value) {
        state.last_event = Some(event.to_string());
        match event {
            "session_started" => {
                state.websocket_state = "connected".into();
                state.task_id = value.get("task_id").and_then(Value::as_str).map(str::to_string);
                state.audio_mode = value.get("audio_mode").and_then(Value::as_str).map(str::to_string);
            }
            "configured" | "voice_switched" => {
                state.websocket_state = "running".into();
                if let Some(voice_name) = value.get("voice_name").and_then(Value::as_str) {
                    state.configured_voice_name = voice_name.to_string();
                }
            }
            "parameters_updated" => {
                state.websocket_state = "running".into();
            }
            "session_completed" => {
                state.websocket_state = "stopped".into();
            }
            "error" => {
                state.websocket_state = "error".into();
                state.last_error = value.get("message").and_then(Value::as_str).map(str::to_string);
            }
            _ => {}
        }
    }
}

fn patch_snapshot(snapshot: &Arc<RwLock<RealtimeStreamSnapshot>>, patch: impl FnOnce(&mut RealtimeStreamSnapshot)) {
    patch(&mut snapshot.write().expect("realtime snapshot lock poisoned"));
}

fn websocket_error(error: tokio_tungstenite::tungstenite::Error) -> AppError {
    AppError::realtime_session(format!("FunSpeech websocket error: {error}"))
}
