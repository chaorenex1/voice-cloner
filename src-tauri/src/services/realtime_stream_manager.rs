use std::{
    collections::{BTreeMap, VecDeque},
    sync::{mpsc as std_mpsc, Arc, Mutex, RwLock},
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
        frame::{AudioFrame, AudioLevel, PcmFormat, SampleFormat},
        virtual_mic::{SelectableVirtualMicAdapter, VirtualMicAdapter},
    },
    domain::{runtime_params::RuntimeParams, session::RealtimeSession},
};

const FUNSPEECH_REALTIME_SAMPLE_RATE: u32 = 16_000;
const PLAYBACK_GAP_SKIP_PENDING: usize = 10;
const PLAYBACK_MAX_PENDING: usize = 600;
const PLAYBACK_JITTER_PREBUFFER_MS: usize = 0;
const REALTIME_LEDGER_LIMIT: usize = 80;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeLedgerEntry {
    pub timestamp_ms: i64,
    pub stage: String,
    pub event: String,
    pub status: Option<String>,
    pub message: Option<String>,
    pub input_frame_seq: Option<u64>,
    pub rust_sent_seq: Option<u64>,
    pub server_dequeued_seq: Option<u64>,
    pub asr_segment_id: Option<String>,
    pub asr_first_frame_seq: Option<u64>,
    pub asr_last_frame_seq: Option<u64>,
    pub asr_commit_reason: Option<String>,
    pub asr_queue_ms: Option<u64>,
    pub tts_revision_id: Option<u64>,
    pub tts_job_id: Option<String>,
    pub audio_chunk_index: Option<u64>,
    pub playback_queue_ms: Option<u64>,
}

impl RealtimeLedgerEntry {
    fn new(stage: impl Into<String>, event: impl Into<String>) -> Self {
        Self {
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            stage: stage.into(),
            event: event.into(),
            status: None,
            message: None,
            input_frame_seq: None,
            rust_sent_seq: None,
            server_dequeued_seq: None,
            asr_segment_id: None,
            asr_first_frame_seq: None,
            asr_last_frame_seq: None,
            asr_commit_reason: None,
            asr_queue_ms: None,
            tts_revision_id: None,
            tts_job_id: None,
            audio_chunk_index: None,
            playback_queue_ms: None,
        }
    }
}

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
    pub input_state: String,
    pub input_source: String,
    pub input_health: Option<String>,
    pub monitor_state: String,
    pub virtual_mic_frames: u64,
    pub monitor_frames: u64,
    pub output_received_frames: u64,
    pub output_written_frames: u64,
    pub output_ack_mismatches: u64,
    pub output_playback_queue_ms: u64,
    pub output_last_frame_gap_ms: Option<u64>,
    pub output_max_frame_gap_ms: Option<u64>,
    pub output_gap_skips: u64,
    pub output_late_drops: u64,
    pub output_overflow_drops: u64,
    pub output_duplicate_drops: u64,
    pub output_playable_frames: u64,
    pub first_output_latency_ms: Option<u64>,
    pub last_output_at_ms: Option<i64>,
    pub rust_sent_seq: Option<u64>,
    pub server_dequeued_seq: Option<u64>,
    pub asr_committed_segments: u64,
    pub asr_committed_audio_ms: u64,
    pub asr_segment_id: Option<String>,
    pub asr_first_frame_seq: Option<u64>,
    pub asr_last_frame_seq: Option<u64>,
    pub asr_commit_reason: Option<String>,
    pub asr_queue_ms: Option<u64>,
    pub ledger: Vec<RealtimeLedgerEntry>,
    pub vad_speech_frames: u64,
    pub vad_utterances_ended: u64,
    pub tts_audio_chunks: u64,
    pub converted_frames: u64,
    pub pipeline_stage: String,
    pub asr_text: Option<String>,
    pub tts_text_chunks: u64,
    pub last_event: Option<String>,
    pub protocol_event: Option<String>,
    pub last_prompt: Option<String>,
    pub event_seq: Option<u64>,
    pub server_ts_ms: Option<i64>,
    pub schema_version: Option<String>,
    pub utterance_id: Option<String>,
    pub hypothesis_id: Option<String>,
    pub revision_id: Option<u64>,
    pub tts_job_id: Option<String>,
    pub audio_chunk_index: Option<u64>,
    pub config_version: Option<u64>,
    pub server_realtime_config: Option<Value>,
    pub asr_committed_text: Option<String>,
    pub asr_committed_chars: u64,
    pub tts_queued_jobs: u64,
    pub tts_started_jobs: u64,
    pub tts_completed_jobs: u64,
    pub tts_dropped_jobs: u64,
    pub tts_queued_chars: u64,
    pub tts_started_chars: u64,
    pub tts_completed_chars: u64,
    pub tts_dropped_chars: u64,
    pub backpressure_hint: Option<String>,
    pub last_error: Option<String>,
}

impl RealtimeStreamSnapshot {
    pub(crate) fn pending(session: &RealtimeSession) -> Self {
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
            input_state: "off".into(),
            input_source: "microphone".into(),
            input_health: None,
            monitor_state: "off".into(),
            virtual_mic_frames: 0,
            monitor_frames: 0,
            output_received_frames: 0,
            output_written_frames: 0,
            output_ack_mismatches: 0,
            output_playback_queue_ms: 0,
            output_last_frame_gap_ms: None,
            output_max_frame_gap_ms: None,
            output_gap_skips: 0,
            output_late_drops: 0,
            output_overflow_drops: 0,
            output_duplicate_drops: 0,
            output_playable_frames: 0,
            first_output_latency_ms: None,
            last_output_at_ms: None,
            rust_sent_seq: None,
            server_dequeued_seq: None,
            asr_committed_segments: 0,
            asr_committed_audio_ms: 0,
            asr_segment_id: None,
            asr_first_frame_seq: None,
            asr_last_frame_seq: None,
            asr_commit_reason: None,
            asr_queue_ms: None,
            ledger: Vec::new(),
            vad_speech_frames: 0,
            vad_utterances_ended: 0,
            tts_audio_chunks: 0,
            converted_frames: 0,
            pipeline_stage: "connecting".into(),
            asr_text: None,
            tts_text_chunks: 0,
            last_event: None,
            protocol_event: None,
            last_prompt: None,
            event_seq: None,
            server_ts_ms: None,
            schema_version: None,
            utterance_id: None,
            hypothesis_id: None,
            revision_id: None,
            tts_job_id: None,
            audio_chunk_index: None,
            config_version: None,
            server_realtime_config: None,
            asr_committed_text: None,
            asr_committed_chars: 0,
            tts_queued_jobs: 0,
            tts_started_jobs: 0,
            tts_completed_jobs: 0,
            tts_dropped_jobs: 0,
            tts_queued_chars: 0,
            tts_started_chars: 0,
            tts_completed_chars: 0,
            tts_dropped_chars: 0,
            backpressure_hint: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RealtimeStreamMode {
    RealtimeVoice,
    AsrTts { asr_url: String, tts_url: String },
}

impl RealtimeStreamMode {
    fn label(&self) -> &'static str {
        match self {
            Self::RealtimeVoice => "realtime_voice",
            Self::AsrTts { .. } => "asr_tts",
        }
    }
}

enum RealtimeControl {
    StartInput(cpal::Device),
    StartFileInput { file_name: String, audio_bytes: Vec<u8> },
    StopInput,
    StartMonitor(cpal::Device),
    StopMonitor,
    UpdateParams(RuntimeParams),
    SwitchVoice(String),
    Stop,
}

impl RealtimeControl {
    fn label(&self) -> &'static str {
        match self {
            Self::StartInput(_) => "start_input",
            Self::StartFileInput { .. } => "start_file_input",
            Self::StopInput => "stop_input",
            Self::StartMonitor(_) => "start_monitor",
            Self::StopMonitor => "stop_monitor",
            Self::UpdateParams(_) => "update_params",
            Self::SwitchVoice(_) => "switch_voice",
            Self::Stop => "stop",
        }
    }
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
        mode: RealtimeStreamMode,
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
            realtime_mode = mode.label(),
            "realtime stream start requested"
        );
        self.stop(&session.session_id).await;

        let snapshot = Arc::new(RwLock::new(RealtimeStreamSnapshot::pending(&session)));
        if let RealtimeStreamMode::AsrTts { asr_url, tts_url } = &mode {
            patch_snapshot(&snapshot, |state| {
                state.websocket_url = format!("{asr_url} -> {tts_url}");
                state.audio_mode = Some("asr_tts_pipeline".into());
            });
        }
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = oneshot::channel();
        let session_id = session.session_id.clone();

        match mode {
            RealtimeStreamMode::RealtimeVoice => {
                tokio::spawn(run_realtime_voice_stream(
                    session,
                    format,
                    input_device,
                    virtual_mic,
                    write_virtual_mic,
                    Arc::clone(&snapshot),
                    control_rx,
                    ready_tx,
                ));
            }
            RealtimeStreamMode::AsrTts { asr_url, tts_url } => {
                tokio::spawn(run_asr_tts_stream(
                    session,
                    format,
                    input_device,
                    virtual_mic,
                    write_virtual_mic,
                    asr_url,
                    tts_url,
                    Arc::clone(&snapshot),
                    control_rx,
                    ready_tx,
                ));
            }
        }

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

    pub fn start_input(&self, session_id: &str, device: cpal::Device) -> AppResult<()> {
        self.patch_and_send_control(
            session_id,
            |state| {
                state.input_state = "starting".into();
                state.input_source = "microphone".into();
                state.input_health = Some("麦克风启动中".into());
            },
            RealtimeControl::StartInput(device),
        )
    }

    pub fn start_file_input(&self, session_id: &str, file_name: String, audio_bytes: Vec<u8>) -> AppResult<()> {
        let display_name = file_name.clone();
        self.patch_and_send_control(
            session_id,
            |state| {
                state.input_state = "starting".into();
                state.input_source = "local_file".into();
                state.input_health = Some(format!("正在读取本地音频: {display_name}"));
            },
            RealtimeControl::StartFileInput { file_name, audio_bytes },
        )
    }

    pub fn stop_input(&self, session_id: &str) -> AppResult<()> {
        self.patch_and_send_control(
            session_id,
            |state| state.input_state = "stopping".into(),
            RealtimeControl::StopInput,
        )
    }

    pub fn start_monitor(&self, session_id: &str, device: cpal::Device) -> AppResult<()> {
        self.patch_and_send_control(
            session_id,
            |state| state.monitor_state = "starting".into(),
            RealtimeControl::StartMonitor(device),
        )
    }

    pub fn stop_monitor(&self, session_id: &str) -> AppResult<()> {
        self.patch_and_send_control(
            session_id,
            |state| state.monitor_state = "stopping".into(),
            RealtimeControl::StopMonitor,
        )
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
        tracing::debug!(%session_id, control = control.label(), "sending realtime stream control");
        let streams = self.streams.read().expect("realtime stream manager lock poisoned");
        let handle = streams
            .get(session_id)
            .ok_or_else(|| AppError::realtime_session(format!("realtime stream not found: {session_id}")))?;
        handle
            .control
            .send(control)
            .map_err(|_| AppError::realtime_session("realtime stream control channel is closed"))
    }

    fn patch_and_send_control(
        &self,
        session_id: &str,
        patch: impl FnOnce(&mut RealtimeStreamSnapshot),
        control: RealtimeControl,
    ) -> AppResult<()> {
        tracing::debug!(%session_id, control = control.label(), "sending realtime stream control");
        let streams = self.streams.read().expect("realtime stream manager lock poisoned");
        let handle = streams
            .get(session_id)
            .ok_or_else(|| AppError::realtime_session(format!("realtime stream not found: {session_id}")))?;
        patch_snapshot(&handle.snapshot, patch);
        handle
            .control
            .send(control)
            .map_err(|_| AppError::realtime_session("realtime stream control channel is closed"))
    }
}

async fn run_realtime_voice_stream(
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
        let funspeech_format = funspeech_realtime_format(format);
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
            "type": "start",
            "voice_name": session.voice_name,
            "voiceName": session.voice_name,
            "pipeline": "asr_tts",
            "format": "pcm",
            "sample_rate": funspeech_format.sample_rate,
            "sampleRate": funspeech_format.sample_rate,
            "params": session.runtime_params.values,
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
            sample_rate = funspeech_format.sample_rate,
            frame_ms = format.frame_ms,
            "FunSpeech configure event sent"
        );

        let mut accepted = false;
        for _ in 0..4 {
            let event = read_json_event(&mut read).await?;
            tracing::debug!(
                session_id = %session.session_id,
                trace_id = %session.trace_id,
                event = ?event_name(&event),
                payload = %event,
                "FunSpeech realtime event received"
            );
            patch_snapshot(&snapshot, |state| apply_json_event(state, &event));
            match event_name(&event) {
                Some("configured" | "ready") => {
                    accepted = true;
                    break;
                }
                Some("error") => {
                    return Err(AppError::realtime_session(format!(
                        "FunSpeech realtime configure failed: {event}"
                    )));
                }
                _ => {}
            }
        }
        if !accepted {
            return Err(AppError::realtime_session(format!(
                "expected configured/ready from FunSpeech after configure for voice {}",
                session.voice_name
            )));
        }

        patch_snapshot(&snapshot, |state| {
            state.websocket_state = "running".into();
            state.pipeline_stage = "realtime_voice_ready".into();
            state.last_error = None;
        });
        if let Some(tx) = ready_tx.take() {
            let _ = tx.send(Ok(()));
        }

        let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<RealtimeAudioChunk>();
        let (file_done_tx, mut file_done_rx) = mpsc::unbounded_channel::<()>();
        let mut input_stop: Option<std_mpsc::Sender<()>> = None;
        let mut file_input_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut monitor_output: Option<RealtimeMonitorPlayer> = None;
        if input_device.is_some() {
            tracing::debug!(
                session_id = %session.session_id,
                trace_id = %session.trace_id,
                "realtime input device is available but capture waits for StartInput control"
            );
        }
        let frame_samples = vec![0.0; format.samples_per_frame()];
        let mut sequence = 0_u64;
        let mut pending_sends = VecDeque::<Instant>::new();
        let mut pending_output_chunks = VecDeque::<PendingOutputChunk>::new();
        let mut pending_client_events = Vec::<Value>::new();
        let mut playback_buffer =
            OrderedPlaybackBuffer::new(
                PLAYBACK_GAP_SKIP_PENDING,
                PLAYBACK_MAX_PENDING,
                PLAYBACK_JITTER_PREBUFFER_MS,
            );
        let mut output_timeline_started_at = Instant::now();
        let mut last_playable_output_at: Option<Instant> = None;
        let mut playback_tick = tokio::time::interval(Duration::from_millis(format.frame_ms.max(1) as u64));
        playback_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = playback_tick.tick() => {
                    let played = drain_one_playback_frame(
                        &mut playback_buffer,
                        format,
                        funspeech_format,
                        &frame_samples,
                        monitor_output.as_ref(),
                        &virtual_mic,
                        write_virtual_mic,
                        &snapshot,
                        &mut last_playable_output_at,
                        output_timeline_started_at,
                        &mut pending_client_events,
                    );
                    if played && !pending_client_events.is_empty() {
                        flush_client_events(&mut write, &mut pending_client_events).await?;
                    }
                }
                Some(chunk) = audio_rx.recv() => {
                    write.send(Message::Binary(chunk.bytes.clone())).await.map_err(websocket_error)?;
                    sequence += 1;
                    pending_sends.push_back(Instant::now());
                    patch_snapshot(&snapshot, |state| {
                        state.sent_frames += 1;
                        state.sent_bytes += chunk.bytes.len() as u64;
                        state.rust_sent_seq = Some(sequence);
                        state.input_level = chunk.level;
                        state.pipeline_stage = "realtime_voice_sending_audio".into();
                        push_ledger(state, "rust_send", "audio_frame_sent", |entry| {
                            entry.rust_sent_seq = Some(sequence);
                            entry.status = Some("sent".into());
                            entry.message = Some(format!("bytes={} rms={:.4}", chunk.bytes.len(), chunk.level.rms));
                        });
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
                        RealtimeControl::StartInput(device) => {
                            if input_stop.is_none() && file_input_task.is_none() {
                                match spawn_input_thread(device, audio_tx.clone(), funspeech_format) {
                                    Ok(stop) => {
                                        input_stop = Some(stop);
                                        output_timeline_started_at = Instant::now();
                                        last_playable_output_at = None;
                                        patch_snapshot(&snapshot, |state| {
                                            reset_output_flow_metrics(state);
                                            state.input_state = "capturing".into();
                                            state.input_source = "microphone".into();
                                            state.input_health = Some("麦克风正在采集".into());
                                            state.pipeline_stage = "realtime_input_capturing".into();
                                            state.last_event = Some("input_started".into());
                                            state.last_error = None;
                                        });
                                    }
                                    Err(error) => {
                                        patch_snapshot(&snapshot, |state| {
                                            state.input_state = "error".into();
                                            state.input_health = Some("麦克风采集启动失败".into());
                                            state.last_error = Some(error.to_string());
                                        });
                                    }
                                }
                            }
                        }
                        RealtimeControl::StartFileInput { file_name, audio_bytes } => {
                            if input_stop.is_none() && file_input_task.is_none() {
                                match spawn_file_input_source(
                                    audio_tx.clone(),
                                    file_done_tx.clone(),
                                    funspeech_format,
                                    audio_bytes,
                                ) {
                                    Ok(task) => {
                                        file_input_task = Some(task);
                                        output_timeline_started_at = Instant::now();
                                        last_playable_output_at = None;
                                        patch_snapshot(&snapshot, |state| {
                                            reset_output_flow_metrics(state);
                                            state.input_state = "capturing".into();
                                            state.input_source = "local_file".into();
                                            state.input_health = Some(format!("正在模拟播放本地音频: {file_name}"));
                                            state.pipeline_stage = "realtime_file_input_capturing".into();
                                            state.last_event = Some("file_input_started".into());
                                            state.last_error = None;
                                        });
                                    }
                                    Err(error) => {
                                        patch_snapshot(&snapshot, |state| {
                                            state.input_state = "error".into();
                                            state.input_source = "local_file".into();
                                            state.input_health = Some("本地音频读取失败".into());
                                            state.last_error = Some(error.to_string());
                                        });
                                    }
                                }
                            }
                        }
                        RealtimeControl::StopInput => {
                            if let Some(stop) = input_stop.take() {
                                let _ = stop.send(());
                            }
                            if let Some(task) = file_input_task.take() {
                                task.abort();
                            }
                            patch_snapshot(&snapshot, |state| {
                                state.input_state = "off".into();
                                state.input_health = Some("输入已停止".into());
                                state.pipeline_stage = "realtime_voice_ready".into();
                                state.last_event = Some("input_stopped".into());
                            });
                        }
                        RealtimeControl::StartMonitor(device) => {
                            if monitor_output.is_none() {
                                match RealtimeMonitorPlayer::start(device, format) {
                                    Ok(output) => {
                                        monitor_output = Some(output);
                                        patch_snapshot(&snapshot, |state| {
                                            state.monitor_state = "listening".into();
                                            state.last_event = Some("monitor_started".into());
                                            state.last_error = None;
                                        });
                                    }
                                    Err(error) => {
                                        patch_snapshot(&snapshot, |state| {
                                            state.monitor_state = "error".into();
                                            state.last_error = Some(error.to_string());
                                        });
                                    }
                                }
                            }
                        }
                        RealtimeControl::StopMonitor => {
                            if let Some(output) = monitor_output.take() {
                                output.stop();
                            }
                            patch_snapshot(&snapshot, |state| {
                                state.monitor_state = "off".into();
                                state.last_event = Some("monitor_stopped".into());
                            });
                        }
                        RealtimeControl::UpdateParams(runtime_params) => {
                            let message = json!({
                                "event": "update",
                                "type": "update_params",
                                "params": runtime_params.values,
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
                                "type": "update_voice",
                                "voice_name": voice_name,
                                "voiceName": voice_name,
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
                            if let Some(stop) = input_stop.take() {
                                let _ = stop.send(());
                            }
                            if let Some(task) = file_input_task.take() {
                                task.abort();
                            }
                            if let Some(output) = monitor_output.take() {
                                output.stop();
                            }
                            let _ = write
                                .send(Message::Text(json!({"event": "stop", "type": "stop"}).to_string()))
                                .await;
                            tracing::debug!(
                                session_id = %session.session_id,
                                trace_id = %session.trace_id,
                                "realtime stop event sent"
                            );
                            patch_snapshot(&snapshot, |state| {
                                state.websocket_state = "stopping".into();
                                state.input_state = "off".into();
                                state.monitor_state = "off".into();
                                state.last_event = Some("stop_requested".into());
                            });
                            break;
                        }
                    }
                }
                Some(()) = file_done_rx.recv(), if file_input_task.is_some() => {
                    file_input_task = None;
                    patch_snapshot(&snapshot, |state| {
                        state.input_state = "off".into();
                        state.input_health = Some("本地音频模拟播放完成".into());
                        state.pipeline_stage = "realtime_file_input_completed".into();
                        state.last_event = Some("file_input_completed".into());
                    });
                }
                message = read.next() => {
                    match message {
                        Some(Ok(Message::Binary(bytes))) => {
                            let output_metadata = pending_output_chunks.pop_front();
                            let latency_ms = pending_sends
                                .pop_front()
                                .map(|sent_at| sent_at.elapsed().as_millis().min(u64::MAX as u128) as u64);
                            let bytes_len = bytes.len();
                            patch_snapshot(&snapshot, |state| {
                                state.received_frames += 1;
                                state.received_bytes += bytes_len as u64;
                                state.output_received_frames += 1;
                                state.output_playback_queue_ms = playback_buffer.queued_ms(format) as u64;
                                state.pipeline_stage = "realtime_voice_buffering_audio".into();
                                push_ledger(state, "output_receive", "audio_binary_received", |entry| {
                                    entry.audio_chunk_index = output_metadata.as_ref().and_then(|metadata| metadata.audio_chunk_index);
                                    entry.tts_job_id = output_metadata.as_ref().and_then(|metadata| metadata.tts_job_id.clone());
                                    entry.playback_queue_ms = Some(playback_buffer.queued_ms(format) as u64);
                                    entry.status = Some("received".into());
                                    entry.message = Some(format!("bytes={bytes_len}"));
                                });
                            });
                            if let Some(metadata) = output_metadata {
                                if metadata.expected_bytes.is_some_and(|expected| expected != bytes_len as u64) {
                                    patch_snapshot(&snapshot, |state| state.output_ack_mismatches += 1);
                                }
                                if let Err(drop) = playback_buffer.enqueue(metadata, bytes, latency_ms) {
                                    patch_snapshot(&snapshot, |state| {
                                        apply_output_drop_status(state, drop.status);
                                        state.output_playback_queue_ms = playback_buffer.queued_ms(format) as u64;
                                        push_ledger(state, "playback", "audio_chunk_dropped", |entry| {
                                            entry.tts_job_id = drop.tts_job_id.clone();
                                            entry.audio_chunk_index = Some(drop.audio_chunk_index);
                                            entry.playback_queue_ms = Some(playback_buffer.queued_ms(format) as u64);
                                            entry.status = Some(drop.status.into());
                                        });
                                    });
                                    if let Some(event) = client_audio_played_event(
                                        drop.chunk_id.clone(),
                                        drop.tts_job_id.clone(),
                                        drop.audio_chunk_index,
                                        drop.status,
                                        playback_buffer.queued_ms(format),
                                    ) {
                                        pending_client_events.push(event);
                                    }
                                    pending_client_events.push(client_audio_backpressure_event(
                                        "drop",
                                        playback_buffer.queued_ms(format),
                                        drop.status,
                                    ));
                                }
                            } else {
                                patch_snapshot(&snapshot, |state| state.output_ack_mismatches += 1);
                            }
                            patch_snapshot(&snapshot, |state| {
                                state.output_playback_queue_ms = playback_buffer.queued_ms(format) as u64;
                            });

                            for drop in playback_buffer.apply_pressure() {
                                patch_snapshot(&snapshot, |state| {
                                    apply_output_drop_status(state, drop.status);
                                    state.output_playback_queue_ms = playback_buffer.queued_ms(format) as u64;
                                    state.backpressure_hint = Some("播放队列压力过高，已跳到最新连续音频窗口".into());
                                    push_ledger(state, "playback", "audio_chunk_dropped", |entry| {
                                        entry.tts_job_id = drop.tts_job_id.clone();
                                        entry.audio_chunk_index = Some(drop.audio_chunk_index);
                                        entry.playback_queue_ms = Some(playback_buffer.queued_ms(format) as u64);
                                        entry.status = Some(drop.status.into());
                                    });
                                });
                                if let Some(event) = client_audio_played_event(
                                    drop.chunk_id.clone(),
                                    drop.tts_job_id.clone(),
                                    drop.audio_chunk_index,
                                    drop.status,
                                    playback_buffer.queued_ms(format),
                                ) {
                                    pending_client_events.push(event);
                                }
                                pending_client_events.push(client_audio_backpressure_event(
                                    "drop",
                                    playback_buffer.queued_ms(format),
                                    drop.status,
                                ));
                            }

                            if !pending_client_events.is_empty() {
                                flush_client_events(&mut write, &mut pending_client_events).await?;
                            }
                        }
                        Some(Ok(Message::Text(text))) => {
                            match serde_json::from_str::<Value>(&text) {
                                Ok(value) => {
                                    if matches!(event_name(&value), Some("tts.audio_chunk")) {
                                        pending_output_chunks.push_back(PendingOutputChunk {
                                            chunk_id: payload_string(&value, "chunk_id"),
                                            tts_job_id: payload_string(&value, "tts_job_id"),
                                            audio_chunk_index: payload_u64(&value, "audio_chunk_index"),
                                            expected_bytes: payload_u64(&value, "bytes"),
                                        });
                                    }
                                    if matches!(event_name(&value), Some("tts.job_completed" | "tts_completed"))
                                        && !pending_client_events.is_empty()
                                    {
                                        flush_client_events(&mut write, &mut pending_client_events).await?;
                                    }
                                    tracing::debug!(
                                        session_id = %session.session_id,
                                        trace_id = %session.trace_id,
                                        event = ?event_name(&value),
                                        payload = %value,
                                        "FunSpeech realtime event received"
                                    );
                                    patch_snapshot(&snapshot, |state| apply_json_event(state, &value));
                                    if matches!(event_name(&value), Some("error" | "session.error")) {
                                        return Err(AppError::realtime_session(format!(
                                            "FunSpeech realtime error: {}",
                                            realtime_event_message(&value)
                                        )));
                                    }
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
            state.input_state = "off".into();
            state.monitor_state = "off".into();
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

#[allow(clippy::too_many_arguments)]
async fn run_asr_tts_stream(
    session: RealtimeSession,
    format: PcmFormat,
    input_device: Option<cpal::Device>,
    virtual_mic: Arc<SelectableVirtualMicAdapter>,
    write_virtual_mic: bool,
    asr_url: String,
    tts_url: String,
    snapshot: Arc<RwLock<RealtimeStreamSnapshot>>,
    mut control_rx: mpsc::UnboundedReceiver<RealtimeControl>,
    ready_tx: oneshot::Sender<AppResult<()>>,
) {
    tracing::debug!(
        session_id = %session.session_id,
        trace_id = %session.trace_id,
        voice_name = %session.voice_name,
        %asr_url,
        %tts_url,
        "ASR -> TTS realtime pipeline task starting"
    );
    let mut ready_tx = Some(ready_tx);
    let result = async {
        let funspeech_format = funspeech_realtime_format(format);
        let (asr_websocket, _) = connect_async(&asr_url)
            .await
            .map_err(|error| AppError::realtime_session(format!("FunSpeech ASR websocket connect failed: {error}")))?;
        let (tts_websocket, _) = connect_async(&tts_url)
            .await
            .map_err(|error| AppError::realtime_session(format!("FunSpeech TTS websocket connect failed: {error}")))?;
        let (mut asr_write, mut asr_read) = asr_websocket.split();
        let (mut tts_write, mut tts_read) = tts_websocket.split();

        let asr_task_id = pipeline_task_id("asr", &session.session_id);
        let tts_task_id = pipeline_task_id("tts", &session.session_id);

        asr_write
            .send(Message::Text(start_transcription_message(&asr_task_id, funspeech_format).to_string()))
            .await
            .map_err(websocket_error)?;
        let asr_started = read_json_event(&mut asr_read).await?;
        patch_snapshot(&snapshot, |state| apply_asr_event(state, &asr_started));
        if aliyun_event_name(&asr_started) != Some("TranscriptionStarted") {
            return Err(AppError::realtime_session(format!(
                "expected TranscriptionStarted from FunSpeech ASR, got {asr_started}"
            )));
        }

        tts_write
            .send(Message::Text(
                start_synthesis_message(&tts_task_id, &session, funspeech_format).to_string(),
            ))
            .await
            .map_err(websocket_error)?;
        let tts_started = read_json_event(&mut tts_read).await?;
        patch_snapshot(&snapshot, |state| apply_tts_event(state, &tts_started));
        if aliyun_event_name(&tts_started) != Some("SynthesisStarted") {
            return Err(AppError::realtime_session(format!(
                "expected SynthesisStarted from FunSpeech TTS, got {tts_started}"
            )));
        }

        patch_snapshot(&snapshot, |state| {
            state.websocket_state = "running".into();
            state.pipeline_stage = "asr_tts_ready".into();
            state.audio_mode = Some("asr_tts_pipeline".into());
            state.last_event = Some("asr_tts_started".into());
            state.last_error = None;
        });
        if let Some(tx) = ready_tx.take() {
            let _ = tx.send(Ok(()));
        }

        let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<RealtimeAudioChunk>();
        let _input_stop = if let Some(device) = input_device {
            Some(spawn_input_thread(device, audio_tx.clone(), funspeech_format)?)
        } else {
            spawn_silence_source(audio_tx.clone(), funspeech_format);
            None
        };
        let frame_samples = vec![0.0; format.samples_per_frame()];
        let mut sequence = 0_u64;
        let mut pending_sends = VecDeque::<Instant>::new();
        let mut last_synthesized_text = String::new();

        loop {
            tokio::select! {
                Some(chunk) = audio_rx.recv() => {
                    asr_write.send(Message::Binary(chunk.bytes.clone())).await.map_err(websocket_error)?;
                    sequence += 1;
                    patch_snapshot(&snapshot, |state| {
                        state.sent_frames += 1;
                        state.sent_bytes += chunk.bytes.len() as u64;
                        state.input_level = chunk.level;
                        state.pipeline_stage = "asr_receiving_audio".into();
                    });
                }
                Some(control) = control_rx.recv() => {
                    match control {
                        RealtimeControl::StartInput(_) => {
                            patch_snapshot(&snapshot, |state| {
                                state.last_event = Some("asr_tts_dynamic_input_ignored".into());
                                state.last_error = Some(
                                    "ASR->TTS standalone pipeline keeps input fixed for the current session".into(),
                                );
                            });
                        }
                        RealtimeControl::StartFileInput { .. } => {
                            patch_snapshot(&snapshot, |state| {
                                state.last_event = Some("asr_tts_dynamic_input_ignored".into());
                                state.last_error = Some(
                                    "ASR->TTS standalone pipeline keeps input fixed for the current session".into(),
                                );
                            });
                        }
                        RealtimeControl::StopInput => {
                            patch_snapshot(&snapshot, |state| {
                                state.last_event = Some("asr_tts_dynamic_input_ignored".into());
                                state.last_error = Some(
                                    "ASR->TTS standalone pipeline stops input when the session stops".into(),
                                );
                            });
                        }
                        RealtimeControl::StartMonitor(_) | RealtimeControl::StopMonitor => {
                            patch_snapshot(&snapshot, |state| {
                                state.monitor_state = "off".into();
                                state.last_event = Some("asr_tts_monitor_ignored".into());
                                state.last_error = Some(
                                    "ASR->TTS standalone pipeline does not own realtime monitor output".into(),
                                );
                            });
                        }
                        RealtimeControl::UpdateParams(runtime_params) => {
                            patch_snapshot(&snapshot, |state| {
                                state.last_event = Some("tts_parameters_update_ignored".into());
                                state.last_error = Some(format!(
                                    "FunSpeech /ws/v1/tts StartSynthesis parameters are fixed for the current session: {:?}",
                                    runtime_params.values
                                ));
                            });
                        }
                        RealtimeControl::SwitchVoice(voice_name) => {
                            patch_snapshot(&snapshot, |state| {
                                state.last_event = Some("tts_voice_switch_ignored".into());
                                state.last_error = Some(format!(
                                    "FunSpeech /ws/v1/tts voice is fixed until the realtime ASR->TTS session restarts: {voice_name}"
                                ));
                            });
                        }
                        RealtimeControl::Stop => {
                            let _ = asr_write
                                .send(Message::Text(stop_transcription_message(&asr_task_id).to_string()))
                                .await;
                            let _ = tts_write
                                .send(Message::Text(stop_synthesis_message(&tts_task_id).to_string()))
                                .await;
                            patch_snapshot(&snapshot, |state| {
                                state.websocket_state = "stopping".into();
                                state.pipeline_stage = "stopping".into();
                                state.last_event = Some("stop_requested".into());
                            });
                            break;
                        }
                    }
                }
                message = asr_read.next() => {
                    match message {
                        Some(Ok(Message::Text(text))) => {
                            match serde_json::from_str::<Value>(&text) {
                                Ok(value) => {
                                    patch_snapshot(&snapshot, |state| apply_asr_event(state, &value));
                                    if is_final_asr_event(&value) {
                                        if let Some(asr_text) = text_from_asr_event(&value) {
                                            let asr_text = asr_text.trim().to_string();
                                            if !asr_text.is_empty() && asr_text != last_synthesized_text {
                                                last_synthesized_text = asr_text.clone();
                                                let message = run_synthesis_message(&tts_task_id, &asr_text);
                                                tts_write.send(Message::Text(message.to_string())).await.map_err(websocket_error)?;
                                                pending_sends.push_back(Instant::now());
                                                patch_snapshot(&snapshot, |state| {
                                                    state.tts_text_chunks += 1;
                                                    state.pipeline_stage = "tts_synthesizing".into();
                                                    state.last_event = Some("tts_text_sent".into());
                                                });
                                            }
                                        }
                                    }
                                }
                                Err(error) => {
                                    patch_snapshot(&snapshot, |state| {
                                        state.last_error = Some(format!("invalid FunSpeech ASR event JSON: {error}"));
                                    });
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            patch_snapshot(&snapshot, |state| {
                                state.websocket_state = "closed".into();
                                state.pipeline_stage = "asr_closed".into();
                                state.last_event = Some("asr_closed".into());
                            });
                            break;
                        }
                        Some(Ok(_)) => {}
                        Some(Err(error)) => return Err(websocket_error(error)),
                    }
                }
                message = tts_read.next() => {
                    match message {
                        Some(Ok(Message::Binary(bytes))) => {
                            let latency_ms = pending_sends
                                .pop_front()
                                .map(|sent_at| sent_at.elapsed().as_millis().min(u64::MAX as u128) as u64);
                            let samples = pcm_i16_bytes_to_f32(&bytes, funspeech_format.samples_per_frame())
                                .map(|samples| {
                                    resample_samples_linear(
                                        &samples,
                                        funspeech_format.sample_rate,
                                        format.sample_rate,
                                    )
                                })
                                .unwrap_or_else(|| frame_samples.clone());
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
                                state.converted_frames += 1;
                                state.latency_ms = latency_ms;
                                state.pipeline_stage = "tts_audio_received".into();
                                if write_virtual_mic && virtual_mic_result.is_ok() {
                                    state.virtual_mic_frames += 1;
                                } else if let Err(error) = &virtual_mic_result {
                                    state.last_error = Some(error.to_string());
                                }
                            });
                        }
                        Some(Ok(Message::Text(text))) => {
                            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                                patch_snapshot(&snapshot, |state| apply_tts_event(state, &value));
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            patch_snapshot(&snapshot, |state| {
                                state.websocket_state = "closed".into();
                                state.pipeline_stage = "tts_closed".into();
                                state.last_event = Some("tts_closed".into());
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
        Ok(())
    }
    .await;

    if let Err(error) = result {
        tracing::warn!(
            session_id = %session.session_id,
            trace_id = %session.trace_id,
            %error,
            "ASR -> TTS realtime pipeline task failed"
        );
        patch_snapshot(&snapshot, |state| {
            state.websocket_state = "error".into();
            state.pipeline_stage = "error".into();
            state.last_error = Some(error.to_string());
        });
        if let Some(tx) = ready_tx.take() {
            let _ = tx.send(Err(error));
        }
    }
}

fn pipeline_task_id(prefix: &str, session_id: &str) -> String {
    let sanitized = session_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();
    format!(
        "{prefix}_{}_{}",
        chrono::Utc::now().timestamp_millis(),
        sanitized.chars().take(12).collect::<String>()
    )
}

fn funspeech_realtime_format(local_format: PcmFormat) -> PcmFormat {
    PcmFormat {
        sample_rate: FUNSPEECH_REALTIME_SAMPLE_RATE,
        channels: 1,
        sample_format: SampleFormat::I16,
        frame_ms: local_format.frame_ms,
    }
}

fn message_id() -> String {
    format!("vc{:x}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default())
}

fn start_transcription_message(task_id: &str, format: PcmFormat) -> Value {
    json!({
        "header": {
            "message_id": message_id(),
            "task_id": task_id,
            "namespace": "SpeechTranscriber",
            "name": "StartTranscription",
        },
        "payload": {
            "format": "pcm",
            "sample_rate": format.sample_rate,
            "enable_intermediate_result": true,
            "enable_punctuation_prediction": true,
            "enable_inverse_text_normalization": true,
            "enable_voice_detection": true,
        },
    })
}

fn start_synthesis_message(task_id: &str, session: &RealtimeSession, format: PcmFormat) -> Value {
    json!({
        "header": {
            "message_id": message_id(),
            "task_id": task_id,
            "namespace": "FlowingSpeechSynthesizer",
            "name": "StartSynthesis",
        },
        "payload": {
            "voice": session.voice_name,
            "format": "PCM",
            "sample_rate": format.sample_rate,
            "volume": runtime_number(&session.runtime_params, "volume").unwrap_or(50.0) as i64,
            "speech_rate": runtime_number(&session.runtime_params, "speechRate")
                .or_else(|| runtime_number(&session.runtime_params, "speech_rate"))
                .unwrap_or(0.0) as i64,
            "pitch_rate": runtime_number(&session.runtime_params, "pitchRate")
                .or_else(|| runtime_number(&session.runtime_params, "pitch_rate"))
                .or_else(|| runtime_number(&session.runtime_params, "pitch"))
                .unwrap_or(0.0) as i64,
            "prompt": runtime_string(&session.runtime_params, "prompt").unwrap_or_default(),
        },
    })
}

fn run_synthesis_message(task_id: &str, text: &str) -> Value {
    json!({
        "header": {
            "message_id": message_id(),
            "task_id": task_id,
            "namespace": "FlowingSpeechSynthesizer",
            "name": "RunSynthesis",
        },
        "payload": {
            "text": text,
        },
    })
}

fn stop_transcription_message(task_id: &str) -> Value {
    json!({
        "header": {
            "message_id": message_id(),
            "task_id": task_id,
            "namespace": "SpeechTranscriber",
            "name": "StopTranscription",
        },
    })
}

fn stop_synthesis_message(task_id: &str) -> Value {
    json!({
        "header": {
            "message_id": message_id(),
            "task_id": task_id,
            "namespace": "FlowingSpeechSynthesizer",
            "name": "StopSynthesis",
        },
    })
}

fn runtime_number(params: &RuntimeParams, key: &str) -> Option<f64> {
    params.values.get(key).and_then(Value::as_f64)
}

fn runtime_string(params: &RuntimeParams, key: &str) -> Option<String> {
    params.values.get(key).and_then(Value::as_str).map(str::to_string)
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn json_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn payload_string(value: &Value, key: &str) -> Option<String> {
    value
        .get("payload")
        .and_then(|payload| payload.get(key))
        .and_then(Value::as_str)
        .or_else(|| value.get(key).and_then(Value::as_str))
        .map(str::to_string)
}

fn payload_u64(value: &Value, key: &str) -> Option<u64> {
    value
        .get("payload")
        .and_then(|payload| payload.get(key))
        .and_then(Value::as_u64)
        .or_else(|| value.get(key).and_then(Value::as_u64))
}

fn payload_bool(value: &Value, key: &str) -> Option<bool> {
    value
        .get("payload")
        .and_then(|payload| payload.get(key))
        .and_then(Value::as_bool)
        .or_else(|| value.get(key).and_then(Value::as_bool))
}

fn payload_value(value: &Value, key: &str) -> Option<Value> {
    value
        .get("payload")
        .and_then(|payload| payload.get(key))
        .or_else(|| value.get(key))
        .cloned()
}

fn payload_text_chars(value: &Value, key: &str) -> u64 {
    payload_u64(value, "text_chars")
        .or_else(|| payload_string(value, key).map(|text| text.chars().count() as u64))
        .unwrap_or(0)
}

fn protocol_event_name(value: &Value) -> Option<String> {
    payload_string(value, "protocol_event")
}

fn realtime_event_message(value: &Value) -> String {
    payload_string(value, "message").unwrap_or_else(|| value.to_string())
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

#[derive(Debug, Clone)]
struct PendingOutputChunk {
    tts_job_id: Option<String>,
    chunk_id: Option<String>,
    audio_chunk_index: Option<u64>,
    expected_bytes: Option<u64>,
}

#[derive(Debug)]
struct PendingPlaybackFrame {
    metadata: PendingOutputChunk,
    bytes: Vec<u8>,
    latency_ms: Option<u64>,
}

#[derive(Debug, PartialEq)]
struct PlaybackDrop {
    tts_job_id: Option<String>,
    chunk_id: Option<String>,
    audio_chunk_index: u64,
    status: &'static str,
}

#[derive(Debug)]
struct OrderedPlaybackBuffer {
    expected_next_index: Option<u64>,
    pending: BTreeMap<u64, PendingPlaybackFrame>,
    gap_skip_pending: usize,
    max_pending: usize,
    prebuffer_ms: usize,
    playback_started: bool,
}

impl OrderedPlaybackBuffer {
    fn new(gap_skip_pending: usize, max_pending: usize, prebuffer_ms: usize) -> Self {
        Self {
            expected_next_index: None,
            pending: BTreeMap::new(),
            gap_skip_pending,
            max_pending: max_pending.max(1),
            prebuffer_ms,
            playback_started: prebuffer_ms == 0,
        }
    }

    fn enqueue(
        &mut self,
        metadata: PendingOutputChunk,
        bytes: Vec<u8>,
        latency_ms: Option<u64>,
    ) -> Result<u64, PlaybackDrop> {
        let index = metadata.audio_chunk_index.unwrap_or_else(|| {
            self.expected_next_index
                .unwrap_or(1)
                .saturating_add(self.pending.len() as u64)
        });
        if self.expected_next_index.is_none() {
            self.expected_next_index = Some(index);
        }
        if self.expected_next_index.is_some_and(|expected| index < expected) {
            return Err(PlaybackDrop {
                tts_job_id: metadata.tts_job_id,
                chunk_id: metadata.chunk_id,
                audio_chunk_index: index,
                status: "late_drop",
            });
        }
        if self.pending.contains_key(&index) {
            return Err(PlaybackDrop {
                tts_job_id: metadata.tts_job_id,
                chunk_id: metadata.chunk_id,
                audio_chunk_index: index,
                status: "duplicate_drop",
            });
        }
        self.pending.insert(
            index,
            PendingPlaybackFrame {
                metadata,
                bytes,
                latency_ms,
            },
        );
        Ok(index)
    }

    fn apply_pressure(&mut self) -> Vec<PlaybackDrop> {
        let mut drops = Vec::new();
        if self.pending.len() >= self.gap_skip_pending {
            if let (Some(expected), Some(first_pending)) =
                (self.expected_next_index, self.pending.keys().next().copied())
            {
                if first_pending > expected && !self.pending.contains_key(&expected) {
                    self.expected_next_index = Some(first_pending);
                    drops.push(PlaybackDrop {
                        tts_job_id: None,
                        chunk_id: None,
                        audio_chunk_index: expected,
                        status: "gap_skip",
                    });
                }
            }
        }

        while self.pending.len() > self.max_pending {
            let Some(index) = self.pending.keys().next().copied() else {
                break;
            };
            let Some(frame) = self.pending.remove(&index) else {
                break;
            };
            if self.expected_next_index.is_some_and(|expected| expected <= index) {
                self.expected_next_index = Some(index.saturating_add(1));
            }
            drops.push(PlaybackDrop {
                tts_job_id: frame.metadata.tts_job_id,
                chunk_id: frame.metadata.chunk_id,
                audio_chunk_index: index,
                status: "playback_overflow_drop",
            });
        }
        drops
    }

    fn pop_playable(&mut self, format: PcmFormat) -> Option<(u64, PendingPlaybackFrame)> {
        if self.expected_next_index.is_none() {
            self.expected_next_index = self.pending.keys().next().copied();
        }
        if !self.playback_started {
            if self.queued_ms(format) < self.prebuffer_ms && self.pending.len() < self.max_pending {
                return None;
            }
            self.playback_started = true;
        }
        let expected = self.expected_next_index?;
        let frame = self.pending.remove(&expected)?;
        self.expected_next_index = Some(expected.saturating_add(1));
        Some((expected, frame))
    }

    fn queued_ms(&self, format: PcmFormat) -> usize {
        self.pending.len() * format.frame_ms as usize
    }
}

#[allow(clippy::too_many_arguments)]
fn drain_one_playback_frame(
    playback_buffer: &mut OrderedPlaybackBuffer,
    format: PcmFormat,
    funspeech_format: PcmFormat,
    frame_samples: &[f32],
    monitor_output: Option<&RealtimeMonitorPlayer>,
    virtual_mic: &Arc<SelectableVirtualMicAdapter>,
    write_virtual_mic: bool,
    snapshot: &Arc<RwLock<RealtimeStreamSnapshot>>,
    last_playable_output_at: &mut Option<Instant>,
    output_timeline_started_at: Instant,
    pending_client_events: &mut Vec<Value>,
) -> bool {
    let Some((expected, playback_frame)) = playback_buffer.pop_playable(format) else {
        return false;
    };
    let PendingPlaybackFrame {
        metadata,
        bytes,
        latency_ms: played_latency_ms,
    } = playback_frame;
    let played_at = Instant::now();
    let frame_gap_ms =
        last_playable_output_at.map(|last| played_at.duration_since(last).as_millis().min(u64::MAX as u128) as u64);
    *last_playable_output_at = Some(played_at);
    let samples = pcm_i16_bytes_to_f32(&bytes, funspeech_format.samples_per_frame())
        .map(|samples| resample_samples_linear(&samples, funspeech_format.sample_rate, format.sample_rate))
        .unwrap_or_else(|| frame_samples.to_vec());
    let frame = AudioFrame {
        sequence: expected,
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
        format,
        samples,
    };
    let virtual_mic_result = if write_virtual_mic {
        virtual_mic.write_frame(&frame)
    } else {
        Ok(())
    };
    let monitor_written = monitor_output.map(|output| output.push_frame(&frame)).unwrap_or(false);
    let output_written = monitor_written || (write_virtual_mic && virtual_mic_result.is_ok());
    patch_snapshot(snapshot, |state| {
        state.converted_frames += 1;
        state.output_playable_frames += 1;
        state.output_playback_queue_ms = playback_buffer.queued_ms(format) as u64;
        state.output_last_frame_gap_ms = frame_gap_ms;
        if let Some(gap_ms) = frame_gap_ms {
            state.output_max_frame_gap_ms = Some(state.output_max_frame_gap_ms.unwrap_or(0).max(gap_ms));
        }
        if state.first_output_latency_ms.is_none() {
            state.first_output_latency_ms =
                Some(output_timeline_started_at.elapsed().as_millis().min(u64::MAX as u128) as u64);
        }
        state.last_output_at_ms = Some(chrono::Utc::now().timestamp_millis());
        state.latency_ms = played_latency_ms;
        state.pipeline_stage = "realtime_voice_received_audio".into();
        state.last_prompt = Some("正在输出转换后语音".into());
        if output_written {
            state.output_written_frames += 1;
        }
        state.tts_job_id = metadata.tts_job_id.clone();
        state.audio_chunk_index = metadata.audio_chunk_index;
        if monitor_written {
            state.monitor_frames += 1;
        }
        if write_virtual_mic && virtual_mic_result.is_ok() {
            state.virtual_mic_frames += 1;
        } else if let Err(error) = &virtual_mic_result {
            state.last_error = Some(error.to_string());
        }
        push_ledger(state, "playback", "client_audio_played_queued", |entry| {
            entry.tts_job_id = metadata.tts_job_id.clone();
            entry.audio_chunk_index = Some(expected);
            entry.playback_queue_ms = Some(playback_buffer.queued_ms(format) as u64);
            entry.status = Some(if output_written { "played" } else { "queued" }.into());
            entry.message = Some(format!(
                "monitor={} virtual_mic={}",
                monitor_written,
                write_virtual_mic && virtual_mic_result.is_ok()
            ));
        });
    });
    if let Some(event) = client_audio_played_event(
        metadata.chunk_id,
        metadata.tts_job_id,
        expected,
        if output_written { "played" } else { "queued" },
        playback_buffer.queued_ms(format),
    ) {
        pending_client_events.push(event);
    }
    true
}

fn client_audio_played_event(
    chunk_id: Option<String>,
    tts_job_id: Option<String>,
    audio_chunk_index: u64,
    status: &str,
    playback_queue_ms: usize,
) -> Option<Value> {
    let chunk_id = chunk_id?;
    Some(json!({
        "event": "client.audio_played",
        "payload": {
            "chunk_id": chunk_id,
            "tts_job_id": tts_job_id,
            "audio_chunk_index": audio_chunk_index,
            "playback_queue_ms": playback_queue_ms,
            "status": status,
            "client_ts_ms": chrono::Utc::now().timestamp_millis(),
        }
    }))
}

fn client_audio_backpressure_event(level: &str, playback_queue_ms: usize, reason: &str) -> Value {
    json!({
        "event": "client.audio_backpressure",
        "payload": {
            "level": level,
            "playback_queue_ms": playback_queue_ms,
            "reason": reason,
            "client_ts_ms": chrono::Utc::now().timestamp_millis(),
        }
    })
}

async fn flush_client_events<S>(write: &mut S, pending_client_events: &mut Vec<Value>) -> AppResult<()>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    for event in std::mem::take(pending_client_events) {
        write
            .send(Message::Text(event.to_string()))
            .await
            .map_err(|error| AppError::realtime_session(format!("FunSpeech websocket send failed: {error}")))?;
    }
    Ok(())
}

#[derive(Debug)]
struct RealtimeInputFrameCutter {
    source_sample_rate: u32,
    target_format: PcmFormat,
    pending_samples: Vec<f32>,
    source_position: f64,
}

impl RealtimeInputFrameCutter {
    fn new(source_sample_rate: u32, target_format: PcmFormat) -> Self {
        Self {
            source_sample_rate,
            target_format,
            pending_samples: Vec::new(),
            source_position: 0.0,
        }
    }

    fn push_mono_samples(&mut self, samples: &[f32]) -> Vec<RealtimeAudioChunk> {
        self.pending_samples
            .extend(samples.iter().map(|sample| sample.clamp(-1.0, 1.0)));
        let target_samples_per_frame = self.target_format.samples_per_frame().max(1);
        let step = self.source_sample_rate as f64 / self.target_format.sample_rate.max(1) as f64;
        let mut chunks = Vec::new();

        loop {
            let last_source_position =
                self.source_position + (target_samples_per_frame.saturating_sub(1) as f64 * step);
            if last_source_position.floor() as usize >= self.pending_samples.len() {
                break;
            }

            let mut frame_samples = Vec::with_capacity(target_samples_per_frame);
            for index in 0..target_samples_per_frame {
                let source_position = self.source_position + index as f64 * step;
                let left = source_position.floor() as usize;
                let right = (left + 1).min(self.pending_samples.len() - 1);
                let fraction = (source_position - left as f64) as f32;
                let sample =
                    self.pending_samples[left] + (self.pending_samples[right] - self.pending_samples[left]) * fraction;
                frame_samples.push(sample.clamp(-1.0, 1.0));
            }

            self.source_position += target_samples_per_frame as f64 * step;
            let consumed = self.source_position.floor() as usize;
            if consumed > 0 {
                let consumed = consumed.min(self.pending_samples.len());
                self.pending_samples.drain(..consumed);
                self.source_position -= consumed as f64;
            }
            chunks.push(realtime_chunk_from_samples(&frame_samples));
        }

        chunks
    }
}

enum RealtimeMonitorCommand {
    Frame { samples: Vec<f32>, source_sample_rate: u32 },
    Stop,
}

struct RealtimeMonitorPlayer {
    tx: std_mpsc::Sender<RealtimeMonitorCommand>,
}

impl RealtimeMonitorPlayer {
    fn start(device: cpal::Device, source_format: PcmFormat) -> AppResult<Self> {
        let (tx, rx) = std_mpsc::channel::<RealtimeMonitorCommand>();
        let (ready_tx, ready_rx) = std_mpsc::channel::<AppResult<()>>();
        thread::Builder::new()
            .name("realtime-monitor-output".into())
            .spawn(move || {
                let mut output = match RealtimeMonitorOutput::start(device, source_format) {
                    Ok(output) => {
                        let _ = ready_tx.send(Ok(()));
                        output
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };

                while let Ok(command) = rx.recv() {
                    match command {
                        RealtimeMonitorCommand::Frame {
                            samples,
                            source_sample_rate,
                        } => output.push_samples(&samples, source_sample_rate),
                        RealtimeMonitorCommand::Stop => break,
                    }
                }
            })
            .map_err(|source| AppError::io("starting realtime monitor output thread", source))?;
        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| AppError::audio("realtime monitor output stream did not start"))??;
        Ok(Self { tx })
    }

    fn push_frame(&self, frame: &AudioFrame) -> bool {
        self.tx
            .send(RealtimeMonitorCommand::Frame {
                samples: frame.samples.clone(),
                source_sample_rate: frame.format.sample_rate,
            })
            .is_ok()
    }

    fn stop(self) {
        let _ = self.tx.send(RealtimeMonitorCommand::Stop);
    }
}

struct RealtimeMonitorOutput {
    buffer: Arc<Mutex<VecDeque<f32>>>,
    output_sample_rate: u32,
    _stream: cpal::Stream,
}

impl RealtimeMonitorOutput {
    fn start(device: cpal::Device, source_format: PcmFormat) -> AppResult<Self> {
        let supported_config = device
            .default_output_config()
            .map_err(|error| AppError::audio(error.to_string()))?;
        let sample_format = supported_config.sample_format();
        let stream_config: StreamConfig = supported_config.into();
        let output_channels = stream_config.channels.max(1) as usize;
        let output_sample_rate = stream_config.sample_rate.0;
        let buffer = Arc::new(Mutex::new(VecDeque::<f32>::new()));
        let stream = build_monitor_stream(
            &device,
            &stream_config,
            sample_format,
            output_channels,
            Arc::clone(&buffer),
        )?;
        stream.play().map_err(|error| AppError::audio(error.to_string()))?;
        tracing::debug!(
            source_sample_rate = source_format.sample_rate,
            output_sample_rate,
            output_channels,
            "realtime monitor output started"
        );
        Ok(Self {
            buffer,
            output_sample_rate,
            _stream: stream,
        })
    }

    fn push_samples(&mut self, samples: &[f32], source_sample_rate: u32) {
        let samples = resample_samples_linear(samples, source_sample_rate, self.output_sample_rate);
        if samples.is_empty() {
            return;
        }
        let max_buffered_samples = self.output_sample_rate as usize * 4;
        let mut buffer = self.buffer.lock().expect("realtime monitor buffer lock poisoned");
        while buffer.len() + samples.len() > max_buffered_samples {
            buffer.pop_front();
        }
        buffer.extend(samples);
    }
}

trait MonitorSample {
    fn from_f32(sample: f32) -> Self;
}

impl MonitorSample for f32 {
    fn from_f32(sample: f32) -> Self {
        sample.clamp(-1.0, 1.0)
    }
}

impl MonitorSample for i16 {
    fn from_f32(sample: f32) -> Self {
        (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
    }
}

impl MonitorSample for u16 {
    fn from_f32(sample: f32) -> Self {
        ((sample.clamp(-1.0, 1.0) + 1.0) * 0.5 * u16::MAX as f32) as u16
    }
}

fn build_monitor_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    sample_format: CpalSampleFormat,
    output_channels: usize,
    buffer: Arc<Mutex<VecDeque<f32>>>,
) -> AppResult<cpal::Stream> {
    let err_fn = |error| tracing::warn!(%error, "realtime monitor output stream error");
    match sample_format {
        CpalSampleFormat::F32 => {
            let buffer = Arc::clone(&buffer);
            device
                .build_output_stream(
                    config,
                    move |data: &mut [f32], _| write_monitor_output(data, output_channels, &buffer),
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
                    move |data: &mut [i16], _| write_monitor_output(data, output_channels, &buffer),
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
                    move |data: &mut [u16], _| write_monitor_output(data, output_channels, &buffer),
                    err_fn,
                    None,
                )
                .map_err(|error| AppError::audio(error.to_string()))
        }
        other => Err(AppError::audio(format!("unsupported output sample format: {other:?}"))),
    }
}

fn write_monitor_output<T: MonitorSample>(
    output: &mut [T],
    output_channels: usize,
    buffer: &Arc<Mutex<VecDeque<f32>>>,
) {
    let mut samples = buffer.lock().expect("realtime monitor buffer lock poisoned");
    for frame in output.chunks_mut(output_channels.max(1)) {
        let sample = samples.pop_front().unwrap_or(0.0);
        for output_sample in frame {
            *output_sample = T::from_f32(sample);
        }
    }
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

fn realtime_chunk_from_samples(samples: &[f32]) -> RealtimeAudioChunk {
    let level = crate::audio::frame::measure_level(samples);
    let mut bytes = Vec::with_capacity(samples.len() * std::mem::size_of::<i16>());
    for sample in samples {
        bytes.extend_from_slice(&((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    RealtimeAudioChunk { bytes, level }
}

fn spawn_file_input_source(
    tx: mpsc::UnboundedSender<RealtimeAudioChunk>,
    done_tx: mpsc::UnboundedSender<()>,
    format: PcmFormat,
    audio_bytes: Vec<u8>,
) -> AppResult<tokio::task::JoinHandle<()>> {
    let chunks = decode_wav_to_realtime_chunks(&audio_bytes, format)?;
    Ok(tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(format.frame_ms.max(1) as u64));
        for chunk in chunks {
            interval.tick().await;
            if tx.send(chunk).is_err() {
                break;
            }
        }
        let _ = done_tx.send(());
    }))
}

fn decode_wav_to_realtime_chunks(audio_bytes: &[u8], format: PcmFormat) -> AppResult<Vec<RealtimeAudioChunk>> {
    let cursor = std::io::Cursor::new(audio_bytes);
    let mut reader = hound::WavReader::new(cursor).map_err(|error| AppError::audio(error.to_string()))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let source_rate = spec.sample_rate;
    let mut interleaved = Vec::<f32>::new();
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for sample in reader.samples::<f32>() {
                interleaved.push(
                    sample
                        .map_err(|error| AppError::audio(error.to_string()))?
                        .clamp(-1.0, 1.0),
                );
            }
        }
        hound::SampleFormat::Int => {
            if spec.bits_per_sample <= 16 {
                for sample in reader.samples::<i16>() {
                    interleaved
                        .push(sample.map_err(|error| AppError::audio(error.to_string()))? as f32 / i16::MAX as f32);
                }
            } else {
                let max_value = ((1_i64 << (spec.bits_per_sample.saturating_sub(1) as u32)) - 1).max(1) as f32;
                for sample in reader.samples::<i32>() {
                    interleaved.push(
                        (sample.map_err(|error| AppError::audio(error.to_string()))? as f32 / max_value)
                            .clamp(-1.0, 1.0),
                    );
                }
            }
        }
    }

    let mut mono = Vec::with_capacity(interleaved.len() / channels.max(1));
    for frame in interleaved.chunks(channels) {
        let mixed = frame.iter().copied().sum::<f32>() / frame.len().max(1) as f32;
        mono.push(mixed.clamp(-1.0, 1.0));
    }
    let resampled = resample_samples_linear(&mono, source_rate, format.sample_rate);
    let samples_per_frame = format.samples_per_frame().max(1);
    let mut chunks = Vec::new();
    for frame in resampled.chunks(samples_per_frame) {
        let mut samples = frame.to_vec();
        if samples.len() < samples_per_frame {
            samples.resize(samples_per_frame, 0.0);
        }
        chunks.push(realtime_chunk_from_samples(&samples));
    }
    if chunks.is_empty() {
        return Err(AppError::audio("local audio file did not contain playable samples"));
    }
    Ok(chunks)
}

fn spawn_input_thread(
    device: cpal::Device,
    tx: mpsc::UnboundedSender<RealtimeAudioChunk>,
    target_format: PcmFormat,
) -> AppResult<std_mpsc::Sender<()>> {
    let (stop_tx, stop_rx) = std_mpsc::channel::<()>();
    let (ready_tx, ready_rx) = std_mpsc::channel::<AppResult<()>>();
    thread::Builder::new()
        .name("realtime-input-capture".into())
        .spawn(move || {
            let result = start_input_stream_on_thread(device, tx, target_format);
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
    target_format: PcmFormat,
) -> AppResult<cpal::Stream> {
    let supported_config = device
        .default_input_config()
        .map_err(|error| AppError::audio(error.to_string()))?;
    let sample_format = supported_config.sample_format();
    let config: StreamConfig = supported_config.into();
    let channels = config.channels.max(1) as usize;
    let input_sample_rate = config.sample_rate.0;
    let err_fn = |error| tracing::warn!(%error, "realtime input stream error");
    let frame_cutter = Arc::new(Mutex::new(RealtimeInputFrameCutter::new(
        input_sample_rate,
        target_format,
    )));

    let stream = match sample_format {
        CpalSampleFormat::F32 => {
            let frame_cutter = Arc::clone(&frame_cutter);
            device.build_input_stream(
                &config,
                move |data: &[f32], _| send_input_chunk(data, channels, &frame_cutter, &tx),
                err_fn,
                None,
            )
        }
        CpalSampleFormat::I16 => {
            let frame_cutter = Arc::clone(&frame_cutter);
            device.build_input_stream(
                &config,
                move |data: &[i16], _| send_input_chunk(data, channels, &frame_cutter, &tx),
                err_fn,
                None,
            )
        }
        CpalSampleFormat::U16 => {
            let frame_cutter = Arc::clone(&frame_cutter);
            device.build_input_stream(
                &config,
                move |data: &[u16], _| send_input_chunk(data, channels, &frame_cutter, &tx),
                err_fn,
                None,
            )
        }
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
    frame_cutter: &Arc<Mutex<RealtimeInputFrameCutter>>,
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
    let chunks = match frame_cutter.lock() {
        Ok(mut cutter) => cutter.push_mono_samples(&mono),
        Err(error) => {
            tracing::warn!(%error, "realtime input frame cutter lock poisoned");
            Vec::new()
        }
    };
    for chunk in chunks {
        let _ = tx.send(chunk);
    }
}

fn resample_samples_linear(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == 0 || target_rate == 0 {
        return Vec::new();
    }
    if source_rate == target_rate {
        return samples.to_vec();
    }

    let target_len = ((samples.len() as u64 * target_rate as u64) / source_rate as u64).max(1) as usize;
    if target_len == 1 {
        return vec![samples[0]];
    }

    let scale = (samples.len() - 1) as f32 / (target_len - 1) as f32;
    (0..target_len)
        .map(|index| {
            let source_position = index as f32 * scale;
            let left = source_position.floor() as usize;
            let right = (left + 1).min(samples.len() - 1);
            let fraction = source_position - left as f32;
            samples[left] + (samples[right] - samples[left]) * fraction
        })
        .collect()
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
    value
        .get("event")
        .and_then(Value::as_str)
        .or_else(|| value.get("type").and_then(Value::as_str))
}

fn aliyun_event_name(value: &Value) -> Option<&str> {
    value
        .get("header")
        .and_then(|header| header.get("name"))
        .and_then(Value::as_str)
        .or_else(|| event_name(value))
}

fn aliyun_status_message(value: &Value) -> Option<String> {
    let header = value.get("header")?;
    header
        .get("status_text")
        .or_else(|| header.get("status_message"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn text_from_asr_event(value: &Value) -> Option<&str> {
    value
        .get("text")
        .or_else(|| value.get("result"))
        .or_else(|| value.get("transcript"))
        .or_else(|| value.get("payload").and_then(|payload| payload.get("text")))
        .or_else(|| value.get("payload").and_then(|payload| payload.get("result")))
        .and_then(Value::as_str)
}

fn is_final_asr_event(value: &Value) -> bool {
    aliyun_event_name(value) == Some("SentenceEnd")
        || value
            .get("payload")
            .and_then(|payload| payload.get("is_final"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn apply_asr_event(state: &mut RealtimeStreamSnapshot, value: &Value) {
    if let Some(event) = aliyun_event_name(value) {
        state.last_event = Some(format!("asr_{event}"));
        match event {
            "TranscriptionStarted" => {
                state.pipeline_stage = "asr_ready".into();
            }
            "TranscriptionResultChanged" | "SentenceEnd" => {
                state.pipeline_stage = "asr_text_received".into();
            }
            "TranscriptionCompleted" => {
                state.pipeline_stage = "asr_completed".into();
            }
            "TaskFailed" => {
                state.websocket_state = "error".into();
                state.last_error = aliyun_status_message(value);
            }
            _ => {}
        }
    }
    if let Some(text) = text_from_asr_event(value) {
        state.asr_text = Some(text.to_string());
        state.pipeline_stage = "asr_text_received".into();
    }
    if event_name(value) == Some("error") {
        state.websocket_state = "error".into();
        state.last_error = value.get("message").and_then(Value::as_str).map(str::to_string);
    }
}

fn apply_tts_event(state: &mut RealtimeStreamSnapshot, value: &Value) {
    if let Some(event) = aliyun_event_name(value) {
        state.last_event = Some(format!("tts_{event}"));
        match event {
            "configured" | "synthesis_started" | "SynthesisStarted" => {
                state.pipeline_stage = "tts_ready".into();
            }
            "synthesis_completed" | "SynthesisCompleted" => {
                state.pipeline_stage = "tts_completed".into();
            }
            "error" | "TaskFailed" => {
                state.websocket_state = "error".into();
                state.last_error = value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| aliyun_status_message(value));
            }
            _ => {}
        }
    }
}

fn apply_output_drop_status(state: &mut RealtimeStreamSnapshot, status: &str) {
    match status {
        "gap_skip" => state.output_gap_skips += 1,
        "late_drop" => state.output_late_drops += 1,
        "playback_overflow_drop" => state.output_overflow_drops += 1,
        "duplicate_drop" => state.output_duplicate_drops += 1,
        _ => {}
    }
}

fn reset_output_flow_metrics(state: &mut RealtimeStreamSnapshot) {
    state.output_playback_queue_ms = 0;
    state.output_last_frame_gap_ms = None;
    state.output_max_frame_gap_ms = None;
    state.output_gap_skips = 0;
    state.output_late_drops = 0;
    state.output_overflow_drops = 0;
    state.output_duplicate_drops = 0;
    state.output_playable_frames = 0;
    state.first_output_latency_ms = None;
    state.last_output_at_ms = None;
}

fn push_ledger(
    state: &mut RealtimeStreamSnapshot,
    stage: impl Into<String>,
    event: impl Into<String>,
    patch: impl FnOnce(&mut RealtimeLedgerEntry),
) {
    let mut entry = RealtimeLedgerEntry::new(stage, event);
    patch(&mut entry);
    if state.ledger.len() >= REALTIME_LEDGER_LIMIT {
        state.ledger.remove(0);
    }
    state.ledger.push(entry);
}

fn apply_json_event(state: &mut RealtimeStreamSnapshot, value: &Value) {
    if let Some(event) = event_name(value) {
        state.last_event = Some(event.to_string());
        state.protocol_event = protocol_event_name(value);
        state.event_seq = json_u64(value, "seq");
        state.server_ts_ms = json_i64(value, "server_ts_ms");
        state.schema_version = json_string(value, "schema_version");
        if let Some(task_id) = json_string(value, "task_id").or_else(|| json_string(value, "session_id")) {
            state.task_id = Some(task_id);
        }
        if let Some(utterance_id) = payload_string(value, "utterance_id") {
            state.utterance_id = Some(utterance_id);
        }
        if let Some(hypothesis_id) = payload_string(value, "hypothesis_id") {
            state.hypothesis_id = Some(hypothesis_id);
        }
        if let Some(revision_id) = payload_u64(value, "revision_id") {
            state.revision_id = Some(revision_id);
        }
        if let Some(tts_job_id) = payload_string(value, "tts_job_id") {
            state.tts_job_id = Some(tts_job_id);
        }
        if let Some(audio_chunk_index) = payload_u64(value, "audio_chunk_index") {
            state.audio_chunk_index = Some(audio_chunk_index);
        }
        if let Some(config_version) = payload_u64(value, "config_version") {
            state.config_version = Some(config_version);
        }

        match event {
            "session_started" => {
                state.websocket_state = "connected".into();
                state.audio_mode = payload_string(value, "audio_mode");
                state.pipeline_stage = "session_started".into();
                state.last_prompt = Some("已连接 FunSpeech，正在准备实时会话".into());
            }
            "configured" | "ready" | "voice_switched" | "voice_updated" => {
                state.websocket_state = "running".into();
                if let Some(config) = payload_value(value, "realtime_config") {
                    state.server_realtime_config = Some(config);
                }
                if let Some(audio_mode) = payload_string(value, "audio_mode") {
                    state.audio_mode = Some(audio_mode);
                }
                if let Some(voice_name) =
                    payload_string(value, "voice_name").or_else(|| payload_string(value, "voiceName"))
                {
                    state.configured_voice_name = voice_name.to_string();
                }
                if matches!(event, "voice_switched" | "voice_updated") {
                    state.pipeline_stage = "voice_switched".into();
                    state.last_prompt = Some("已切换音色".into());
                } else {
                    state.pipeline_stage = "configured".into();
                    state.last_prompt = Some("音色已就绪，可以打开麦克风".into());
                }
            }
            "parameters_updated" | "params_updated" => {
                state.websocket_state = "running".into();
                state.pipeline_stage = "parameters_updated".into();
                state.last_prompt = Some("实时参数已生效".into());
            }
            "session_completed" | "closed" => {
                state.websocket_state = "stopped".into();
                state.pipeline_stage = "completed".into();
                state.last_prompt = Some("实时会话已结束".into());
            }
            "session.error" | "error" => {
                state.websocket_state = "error".into();
                state.last_error = payload_string(value, "message");
                state.last_prompt = state.last_error.clone().or_else(|| Some("实时会话发生错误".into()));
            }
            "input.audio_dequeued" => {
                if let Some(seq) = payload_u64(value, "input_frame_index") {
                    state.server_dequeued_seq = Some(seq);
                }
                state.pipeline_stage = "input_audio_dequeued".into();
                if let Some(is_silence) = payload_bool(value, "is_silence") {
                    state.input_health = Some(if is_silence {
                        "服务端收到输入，但判定为静音".into()
                    } else {
                        "服务端正在接收有效输入".into()
                    });
                }
                let server_dequeued_seq = state.server_dequeued_seq;
                let input_message = state.input_health.clone();
                push_ledger(state, "server_input", event, |entry| {
                    entry.server_dequeued_seq = server_dequeued_seq;
                    entry.playback_queue_ms = payload_u64(value, "queue_ms");
                    entry.status = payload_string(value, "vad_state");
                    entry.message = input_message;
                });
                state.last_prompt = Some("正在接收麦克风输入".into());
            }
            "vad.speech_frame" => {
                state.vad_speech_frames += payload_u64(value, "frames").unwrap_or(1);
                state.pipeline_stage = "vad_speech_frame".into();
                state.input_health = Some("VAD 检测到有效语音帧".into());
                state.last_prompt = Some("VAD 检测到语音，正在送入 ASR".into());
            }
            "vad.speech_started" => {
                state.vad_speech_frames += 1;
                state.pipeline_stage = "vad_speech_started".into();
                state.input_health = Some("VAD 检测到开始说话".into());
                state.last_prompt = Some("检测到开始说话".into());
            }
            "vad.speech_ended" => {
                state.vad_utterances_ended += 1;
                state.pipeline_stage = "vad_speech_ended".into();
                state.input_health = Some("VAD 检测到本句结束".into());
                state.last_prompt = Some("本句语音结束，等待变声输出".into());
            }
            "asr.segment_committed" => {
                state.asr_committed_segments += 1;
                state.asr_committed_audio_ms += payload_u64(value, "duration_ms").unwrap_or_default();
                state.asr_segment_id = payload_string(value, "segment_id");
                state.asr_first_frame_seq = payload_u64(value, "first_input_frame_index");
                state.asr_last_frame_seq = payload_u64(value, "last_input_frame_index");
                state.asr_commit_reason = payload_string(value, "commit_reason");
                state.asr_queue_ms = payload_u64(value, "queue_ms");
                state.pipeline_stage = "asr_segment_committed".into();
                state.last_prompt = Some("ASR 连续语音段已入队".into());
                let asr_segment_id = state.asr_segment_id.clone();
                let asr_first_frame_seq = state.asr_first_frame_seq;
                let asr_last_frame_seq = state.asr_last_frame_seq;
                let asr_commit_reason = state.asr_commit_reason.clone();
                let asr_queue_ms = state.asr_queue_ms;
                push_ledger(state, "asr_segment", event, |entry| {
                    entry.asr_segment_id = asr_segment_id;
                    entry.asr_first_frame_seq = asr_first_frame_seq;
                    entry.asr_last_frame_seq = asr_last_frame_seq;
                    entry.asr_commit_reason = asr_commit_reason;
                    entry.asr_queue_ms = asr_queue_ms;
                    entry.status = Some(if payload_bool(value, "is_final").unwrap_or(false) {
                        "final".into()
                    } else {
                        "partial".into()
                    });
                    entry.message = Some(format!(
                        "duration={}ms frames={}",
                        payload_u64(value, "duration_ms").unwrap_or_default(),
                        payload_u64(value, "frame_count").unwrap_or_default()
                    ));
                });
            }
            "backpressure.applied" => {
                let reason = payload_string(value, "reason").unwrap_or_else(|| "backpressure".into());
                let message = payload_string(value, "message").unwrap_or_else(|| "处理压力较高，可能有轻微延迟".into());
                state.backpressure_hint = Some(if message.trim().is_empty() { reason } else { message });
                state.pipeline_stage = "backpressure_applied".into();
                let backpressure_hint = state.backpressure_hint.clone();
                push_ledger(state, "backpressure", event, |entry| {
                    entry.status = payload_string(value, "reason");
                    entry.message = backpressure_hint;
                    entry.asr_queue_ms = payload_u64(value, "queue_ms");
                    entry.asr_first_frame_seq = payload_u64(value, "first_dropped_seq");
                    entry.asr_last_frame_seq = payload_u64(value, "last_dropped_seq");
                });
                state.last_prompt = Some("处理压力较高，可能有轻微延迟".into());
            }
            "asr.hypothesis" | "asr_result" => {
                if let Some(text) = payload_string(value, "text") {
                    state.asr_text = Some(text);
                }
                state.pipeline_stage = if payload_bool(value, "is_final").unwrap_or(false) {
                    "asr_final".into()
                } else {
                    "asr_recognizing".into()
                };
                state.last_prompt = Some("正在识别语音".into());
            }
            "asr.text_committed" => {
                if let Some(delta_text) = payload_string(value, "delta_text") {
                    let committed = state.asr_committed_text.get_or_insert_with(String::new);
                    committed.push_str(&delta_text);
                    state.asr_committed_chars += delta_text.chars().count() as u64;
                    state.asr_text = Some(committed.clone());
                } else if let Some(text) = payload_string(value, "full_text") {
                    state.asr_committed_chars = text.chars().count() as u64;
                    state.asr_committed_text = Some(text.clone());
                    state.asr_text = Some(text);
                }
                state.pipeline_stage = "text_committed".into();
                let revision_id = state.revision_id;
                let tts_job_id = state.tts_job_id.clone();
                push_ledger(state, "asr_text", event, |entry| {
                    entry.tts_revision_id = revision_id;
                    entry.tts_job_id = tts_job_id;
                    entry.message = payload_string(value, "delta_text").or_else(|| payload_string(value, "full_text"));
                    entry.status = Some(if payload_bool(value, "is_final").unwrap_or(false) {
                        "final".into()
                    } else {
                        "stable".into()
                    });
                });
                state.last_prompt = Some("已确认文本，正在准备变声".into());
            }
            "asr.sentence_finalized" => {
                if let Some(text) = payload_string(value, "text") {
                    state.asr_text = Some(text);
                }
                state.pipeline_stage = "sentence_finalized".into();
                state.last_prompt = Some("本句语音识别完成".into());
            }
            "tts.job_queued" => {
                state.tts_queued_jobs += 1;
                state.tts_queued_chars += payload_text_chars(value, "text");
                state.pipeline_stage = "tts_queued".into();
                let revision_id = state.revision_id;
                let tts_job_id = state.tts_job_id.clone();
                push_ledger(state, "tts_queue", event, |entry| {
                    entry.tts_revision_id = revision_id;
                    entry.tts_job_id = tts_job_id;
                    entry.status = payload_string(value, "priority");
                    entry.message = payload_string(value, "text");
                });
                state.last_prompt = Some("变声任务已排队".into());
            }
            "tts.job_started" => {
                state.tts_started_jobs += 1;
                state.tts_started_chars += payload_text_chars(value, "text");
                state.pipeline_stage = "tts_synthesizing".into();
                let revision_id = state.revision_id;
                let tts_job_id = state.tts_job_id.clone();
                push_ledger(state, "tts_synth", event, |entry| {
                    entry.tts_revision_id = revision_id;
                    entry.tts_job_id = tts_job_id;
                    entry.status = payload_string(value, "priority");
                    entry.message = payload_string(value, "text");
                });
                state.last_prompt = Some("正在合成目标音色".into());
            }
            "tts.first_audio" => {
                state.pipeline_stage = "tts_first_audio".into();
                state.last_prompt = Some("即将播放转换后语音".into());
            }
            "tts.audio_chunk" => {
                state.tts_audio_chunks += 1;
                state.pipeline_stage = "tts_audio_chunk_ready".into();
                let revision_id = state.revision_id;
                let tts_job_id = state.tts_job_id.clone();
                let audio_chunk_index = state.audio_chunk_index;
                push_ledger(state, "output_metadata", event, |entry| {
                    entry.tts_revision_id = revision_id;
                    entry.tts_job_id = tts_job_id;
                    entry.audio_chunk_index = audio_chunk_index;
                    entry.status = Some("metadata".into());
                    entry.message = Some(format!("bytes={}", payload_u64(value, "bytes").unwrap_or_default()));
                });
                state.last_prompt = Some("正在输出转换后语音".into());
            }
            "client.audio_ack.received" => {
                state.pipeline_stage = "client_audio_ack_received".into();
                push_ledger(state, "playback_ack", event, |entry| {
                    entry.playback_queue_ms = payload_u64(value, "playback_queue_ms");
                    entry.audio_chunk_index = payload_u64(value, "audio_chunk_index");
                    entry.status = Some("server_received".into());
                });
                state.last_prompt = Some("服务端已确认客户端收到变声音频".into());
            }
            "tts.job_completed" | "tts_completed" => {
                if event == "tts.job_completed" {
                    state.tts_completed_jobs += 1;
                    state.tts_completed_chars += payload_text_chars(value, "text");
                }
                state.pipeline_stage = "tts_completed".into();
                let revision_id = state.revision_id;
                let tts_job_id = state.tts_job_id.clone();
                push_ledger(state, "tts_synth", event, |entry| {
                    entry.tts_revision_id = revision_id;
                    entry.tts_job_id = tts_job_id;
                    entry.status = Some("completed".into());
                    entry.message = payload_string(value, "text");
                });
                state.last_prompt = Some("本段语音转换完成".into());
            }
            "tts.job_dropped" => {
                state.tts_dropped_jobs += 1;
                state.tts_dropped_chars += payload_text_chars(value, "text");
                state.pipeline_stage = "tts_dropped".into();
                let revision_id = state.revision_id;
                let tts_job_id = state.tts_job_id.clone();
                push_ledger(state, "tts_queue", event, |entry| {
                    entry.tts_revision_id = revision_id;
                    entry.tts_job_id = tts_job_id;
                    entry.status = payload_string(value, "reason").or_else(|| Some("dropped".into()));
                    entry.message = payload_string(value, "text");
                });
                state.last_prompt = Some("变声任务被服务端丢弃".into());
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        audio::frame::PcmFormat,
        domain::{
            runtime_params::RuntimeParams,
            session::{RealtimeSession, RealtimeSessionStatus},
        },
    };

    use super::{
        apply_asr_event, apply_json_event, apply_output_drop_status, apply_tts_event, client_audio_backpressure_event,
        client_audio_played_event, funspeech_realtime_format, is_final_asr_event, resample_samples_linear,
        run_synthesis_message, start_synthesis_message, start_transcription_message, text_from_asr_event,
        OrderedPlaybackBuffer, PendingOutputChunk, RealtimeInputFrameCutter, RealtimeStreamSnapshot,
        PLAYBACK_GAP_SKIP_PENDING, PLAYBACK_JITTER_PREBUFFER_MS, PLAYBACK_MAX_PENDING,
    };

    #[test]
    fn asr_tts_messages_follow_funspeech_aliyun_protocol() {
        let local_format = PcmFormat {
            sample_rate: 48_000,
            ..Default::default()
        };
        let funspeech_format = funspeech_realtime_format(local_format);
        let session = RealtimeSession {
            session_id: "session-1".into(),
            trace_id: "trace-1".into(),
            voice_name: "desktop_voice".into(),
            runtime_params: RuntimeParams::default(),
            status: RealtimeSessionStatus::Running,
            websocket_url: "ws://localhost:8000/ws/v1/realtime/voice".into(),
            error_summary: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let asr_start = start_transcription_message("asr-task", funspeech_format);
        assert_eq!(asr_start["header"]["namespace"], "SpeechTranscriber");
        assert_eq!(asr_start["header"]["name"], "StartTranscription");
        assert_eq!(asr_start["payload"]["format"], "pcm");
        assert_eq!(asr_start["payload"]["sample_rate"], 16_000);

        let tts_start = start_synthesis_message("tts-task", &session, funspeech_format);
        assert_eq!(tts_start["header"]["namespace"], "FlowingSpeechSynthesizer");
        assert_eq!(tts_start["header"]["name"], "StartSynthesis");
        assert_eq!(tts_start["payload"]["voice"], "desktop_voice");
        assert_eq!(tts_start["payload"]["format"], "PCM");
        assert_eq!(tts_start["payload"]["sample_rate"], 16_000);

        let run = run_synthesis_message("tts-task", "你好");
        assert_eq!(run["header"]["name"], "RunSynthesis");
        assert_eq!(run["payload"]["text"], "你好");
    }

    #[test]
    fn realtime_resampler_converts_between_local_and_funspeech_rates() {
        let source = vec![0.0, 0.5, 1.0, 0.5, 0.0, -0.5];

        let downsampled = resample_samples_linear(&source, 48_000, 16_000);
        assert_eq!(downsampled.len(), 2);

        let upsampled = resample_samples_linear(&downsampled, 16_000, 48_000);
        assert_eq!(upsampled.len(), 6);
        assert!((upsampled[0] - source[0]).abs() < 0.001);
    }

    #[test]
    fn realtime_input_frame_cutter_emits_fixed_funspeech_frames() {
        let target_format = PcmFormat {
            sample_rate: 16_000,
            channels: 1,
            frame_ms: 20,
            ..Default::default()
        };
        let mut cutter = RealtimeInputFrameCutter::new(48_000, target_format);
        let mut chunks = Vec::new();

        chunks.extend(cutter.push_mono_samples(&vec![0.1; 481]));
        chunks.extend(cutter.push_mono_samples(&vec![0.1; 479]));
        chunks.extend(cutter.push_mono_samples(&vec![0.1; 960]));

        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().all(|chunk| chunk.bytes.len() == 640));
        assert!(chunks.iter().all(|chunk| chunk.level.rms > 0.0));
    }

    #[test]
    fn realtime_client_control_events_match_funspeech_protocol() {
        assert_eq!(PLAYBACK_JITTER_PREBUFFER_MS, 0);

        let played = client_audio_played_event(Some("tts_1_chunk_1".into()), Some("tts_1".into()), 1, "played", 0)
            .expect("chunk_id is required for FunSpeech playback flow control");
        assert_eq!(played["event"], "client.audio_played");
        assert_eq!(played["payload"]["chunk_id"], "tts_1_chunk_1");
        assert_eq!(played["payload"]["tts_job_id"], "tts_1");
        assert_eq!(played["payload"]["audio_chunk_index"], 1);

        let backpressure = client_audio_backpressure_event("drop", 120, "playback_overflow_drop");
        assert_eq!(backpressure["event"], "client.audio_backpressure");
        assert_eq!(backpressure["payload"]["level"], "drop");
        assert_eq!(backpressure["payload"]["playback_queue_ms"], 120);
        assert_eq!(backpressure["payload"]["reason"], "playback_overflow_drop");

        assert!(client_audio_played_event(None, Some("tts_1".into()), 1, "played", 0).is_none());
    }

    #[test]
    fn ordered_playback_buffer_waits_for_contiguous_audio_before_playing() {
        let format = PcmFormat::default();
        let mut buffer = OrderedPlaybackBuffer::new(PLAYBACK_GAP_SKIP_PENDING, PLAYBACK_MAX_PENDING, 0);

        buffer
            .enqueue(
                PendingOutputChunk {
                    chunk_id: None,
                    tts_job_id: Some("tts_1".into()),
                    audio_chunk_index: Some(1),
                    expected_bytes: Some(2),
                },
                vec![1, 1],
                Some(10),
            )
            .expect("chunk 1 should queue");
        let (index, _) = buffer.pop_playable(format).expect("chunk 1 should play");
        assert_eq!(index, 1);

        buffer
            .enqueue(
                PendingOutputChunk {
                    chunk_id: None,
                    tts_job_id: Some("tts_1".into()),
                    audio_chunk_index: Some(3),
                    expected_bytes: Some(2),
                },
                vec![3, 3],
                Some(30),
            )
            .expect("chunk 3 should queue");
        assert!(buffer.pop_playable(format).is_none());
    }

    #[test]
    fn ordered_playback_buffer_skips_gap_when_playback_pressure_builds() {
        let format = PcmFormat::default();
        let mut buffer = OrderedPlaybackBuffer::new(2, PLAYBACK_MAX_PENDING, 0);

        buffer
            .enqueue(
                PendingOutputChunk {
                    chunk_id: None,
                    tts_job_id: Some("tts_1".into()),
                    audio_chunk_index: Some(1),
                    expected_bytes: Some(2),
                },
                vec![1, 1],
                Some(10),
            )
            .expect("chunk 1 should queue");
        let (index, _) = buffer.pop_playable(format).expect("chunk 1 should play");
        assert_eq!(index, 1);

        buffer
            .enqueue(
                PendingOutputChunk {
                    chunk_id: None,
                    tts_job_id: Some("tts_1".into()),
                    audio_chunk_index: Some(4),
                    expected_bytes: Some(2),
                },
                vec![4, 4],
                Some(40),
            )
            .expect("chunk 4 should queue");
        buffer
            .enqueue(
                PendingOutputChunk {
                    chunk_id: None,
                    tts_job_id: Some("tts_1".into()),
                    audio_chunk_index: Some(5),
                    expected_bytes: Some(2),
                },
                vec![5, 5],
                Some(50),
            )
            .expect("chunk 5 should queue");

        let drops = buffer.apply_pressure();
        assert_eq!(drops[0].status, "gap_skip");
        assert_eq!(drops[0].audio_chunk_index, 2);
        let (index, frame) = buffer.pop_playable(format).expect("chunk 4 should play after gap skip");
        assert_eq!(index, 4);
        assert_eq!(frame.bytes, vec![4, 4]);
    }

    #[test]
    fn ordered_playback_buffer_waits_for_fixed_jitter_prebuffer() {
        let format = PcmFormat {
            frame_ms: 20,
            ..Default::default()
        };
        let mut buffer = OrderedPlaybackBuffer::new(PLAYBACK_GAP_SKIP_PENDING, PLAYBACK_MAX_PENDING, 40);

        buffer
            .enqueue(
                PendingOutputChunk {
                    chunk_id: None,
                    tts_job_id: Some("tts_1".into()),
                    audio_chunk_index: Some(1),
                    expected_bytes: Some(2),
                },
                vec![1, 1],
                Some(10),
            )
            .expect("chunk 1 should queue");
        assert!(buffer.pop_playable(format).is_none());

        buffer
            .enqueue(
                PendingOutputChunk {
                    chunk_id: None,
                    tts_job_id: Some("tts_1".into()),
                    audio_chunk_index: Some(2),
                    expected_bytes: Some(2),
                },
                vec![2, 2],
                Some(20),
            )
            .expect("chunk 2 should queue");
        let (index, _) = buffer.pop_playable(format).expect("prebuffer is full");
        assert_eq!(index, 1);
    }

    #[test]
    fn realtime_snapshot_tracks_output_drop_statuses_separately() {
        let session = RealtimeSession {
            session_id: "session-1".into(),
            trace_id: "trace-1".into(),
            voice_name: "desktop_voice".into(),
            runtime_params: RuntimeParams::default(),
            status: RealtimeSessionStatus::Running,
            websocket_url: "ws://localhost:8000/ws/v1/realtime/voice".into(),
            error_summary: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let mut snapshot = RealtimeStreamSnapshot::pending(&session);

        apply_output_drop_status(&mut snapshot, "gap_skip");
        apply_output_drop_status(&mut snapshot, "late_drop");
        apply_output_drop_status(&mut snapshot, "playback_overflow_drop");
        apply_output_drop_status(&mut snapshot, "duplicate_drop");

        assert_eq!(snapshot.output_gap_skips, 1);
        assert_eq!(snapshot.output_late_drops, 1);
        assert_eq!(snapshot.output_overflow_drops, 1);
        assert_eq!(snapshot.output_duplicate_drops, 1);
        assert_eq!(snapshot.output_ack_mismatches, 0);
    }

    #[test]
    fn aliyun_asr_and_tts_events_update_snapshot() {
        let session = RealtimeSession {
            session_id: "session-1".into(),
            trace_id: "trace-1".into(),
            voice_name: "desktop_voice".into(),
            runtime_params: RuntimeParams::default(),
            status: RealtimeSessionStatus::Running,
            websocket_url: "ws://localhost:8000/ws/v1/realtime/voice".into(),
            error_summary: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let mut snapshot = RealtimeStreamSnapshot::pending(&session);
        let asr_final = json!({
            "header": {"name": "SentenceEnd"},
            "payload": {"text": "你好", "is_final": true}
        });

        assert!(is_final_asr_event(&asr_final));
        assert_eq!(text_from_asr_event(&asr_final), Some("你好"));
        apply_asr_event(&mut snapshot, &asr_final);
        assert_eq!(snapshot.asr_text.as_deref(), Some("你好"));
        assert_eq!(snapshot.last_event.as_deref(), Some("asr_SentenceEnd"));

        apply_tts_event(&mut snapshot, &json!({"header": {"name": "SynthesisStarted"}}));
        assert_eq!(snapshot.pipeline_stage, "tts_ready");
        assert_eq!(snapshot.last_event.as_deref(), Some("tts_SynthesisStarted"));
    }

    #[test]
    fn realtime_voice_events_accept_type_protocol_aliases() {
        let session = RealtimeSession {
            session_id: "session-1".into(),
            trace_id: "trace-1".into(),
            voice_name: "desktop_voice".into(),
            runtime_params: RuntimeParams::default(),
            status: RealtimeSessionStatus::Running,
            websocket_url: "ws://localhost:8000/ws/v1/realtime/voice".into(),
            error_summary: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let mut snapshot = RealtimeStreamSnapshot::pending(&session);

        apply_json_event(
            &mut snapshot,
            &json!({"type": "ready", "session_id": "rt-1", "voiceName": "robot"}),
        );
        assert_eq!(snapshot.websocket_state, "running");
        assert_eq!(snapshot.task_id.as_deref(), Some("rt-1"));
        assert_eq!(snapshot.configured_voice_name, "robot");

        apply_json_event(&mut snapshot, &json!({"type": "voice_updated", "voice_name": "girl"}));
        assert_eq!(snapshot.configured_voice_name, "girl");
        assert_eq!(snapshot.last_event.as_deref(), Some("voice_updated"));
    }

    #[test]
    fn realtime_voice_v1_events_update_lightweight_prompt_fields() {
        let session = RealtimeSession {
            session_id: "session-1".into(),
            trace_id: "trace-1".into(),
            voice_name: "desktop_voice".into(),
            runtime_params: RuntimeParams::default(),
            status: RealtimeSessionStatus::Running,
            websocket_url: "ws://localhost:8000/ws/v1/realtime/voice".into(),
            error_summary: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let mut snapshot = RealtimeStreamSnapshot::pending(&session);

        apply_json_event(
            &mut snapshot,
            &json!({
                "event": "asr.text_committed",
                "task_id": "realtime_voice_1",
                "seq": 7,
                "server_ts_ms": 1730000000000i64,
                "schema_version": "realtime_voice.v1",
                "payload": {
                    "utterance_id": "utt_1",
                    "revision_id": 3,
                    "full_text": "你好",
                    "tts_job_id": "tts_3"
                }
            }),
        );

        assert_eq!(snapshot.task_id.as_deref(), Some("realtime_voice_1"));
        assert_eq!(snapshot.event_seq, Some(7));
        assert_eq!(snapshot.schema_version.as_deref(), Some("realtime_voice.v1"));
        assert_eq!(snapshot.utterance_id.as_deref(), Some("utt_1"));
        assert_eq!(snapshot.revision_id, Some(3));
        assert_eq!(snapshot.tts_job_id.as_deref(), Some("tts_3"));
        assert_eq!(snapshot.asr_text.as_deref(), Some("你好"));
        assert_eq!(snapshot.last_prompt.as_deref(), Some("已确认文本，正在准备变声"));

        apply_json_event(
            &mut snapshot,
            &json!({
                "event": "tts.audio_chunk",
                "payload": {
                    "tts_job_id": "tts_3",
                    "revision_id": 3,
                    "audio_chunk_index": 2
                }
            }),
        );

        assert_eq!(snapshot.audio_chunk_index, Some(2));
        assert_eq!(snapshot.last_prompt.as_deref(), Some("正在输出转换后语音"));

        apply_json_event(
            &mut snapshot,
            &json!({
                "event": "asr.segment_committed",
                "payload": {
                    "segment_id": "utt_1_seg_3",
                    "utterance_id": "utt_1",
                    "first_input_frame_index": 41,
                    "last_input_frame_index": 80,
                    "duration_ms": 800,
                    "frame_count": 40,
                    "is_final": false,
                    "commit_reason": "partial",
                    "queue_ms": 120
                }
            }),
        );

        assert_eq!(snapshot.asr_committed_segments, 1);
        assert_eq!(snapshot.asr_segment_id.as_deref(), Some("utt_1_seg_3"));
        assert_eq!(snapshot.asr_first_frame_seq, Some(41));
        assert_eq!(snapshot.asr_last_frame_seq, Some(80));
        assert_eq!(snapshot.asr_queue_ms, Some(120));
        assert!(snapshot
            .ledger
            .iter()
            .any(|entry| entry.stage == "asr_segment" && entry.asr_first_frame_seq == Some(41)));

        apply_json_event(
            &mut snapshot,
            &json!({"event": "vad.speech_started", "payload": {"utterance_id": "utt_2"}}),
        );
        assert_eq!(snapshot.vad_speech_frames, 1);
    }
}
