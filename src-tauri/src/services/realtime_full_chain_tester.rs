use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{
    app::{
        error::{AppError, AppResult},
        state::AppState,
    },
    audio::{
        frame::{PcmFormat, SampleFormat},
        virtual_mic::VirtualMicAdapter,
    },
    domain::{runtime_params::RuntimeParams, settings::AppSettings},
    services::{
        realtime_stream_manager::{RealtimeStreamMode, RealtimeStreamSnapshot},
        session_manager::CreateRealtimeSessionRequest,
    },
};

const DEFAULT_POLL_INTERVAL_MS: u64 = 500;
const DEFAULT_DRAIN_GRACE_MS: u64 = 6_000;
const DEFAULT_MAX_DURATION_MS: u64 = 180_000;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeFullChainTestRequest {
    pub voice_name: String,
    pub file_name: String,
    pub audio_bytes: Vec<u8>,
    #[serde(default)]
    pub runtime_params: RuntimeParams,
    #[serde(default)]
    pub backend_base_url: Option<String>,
    #[serde(default)]
    pub start_monitor: Option<bool>,
    #[serde(default)]
    pub poll_interval_ms: Option<u64>,
    #[serde(default)]
    pub drain_grace_ms: Option<u64>,
    #[serde(default)]
    pub max_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeFullChainTimelineSample {
    pub elapsed_ms: u64,
    pub snapshot: RealtimeStreamSnapshot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeFullChainSummary {
    pub verdict: RealtimeFullChainVerdict,
    pub reasons: Vec<String>,
    pub duration_ms: u64,
    pub sent_frames: u64,
    pub received_frames: u64,
    pub output_received_frames: u64,
    pub output_playable_frames: u64,
    pub output_written_frames: u64,
    pub monitor_frames: u64,
    pub virtual_mic_frames: u64,
    pub output_ack_mismatches: u64,
    pub output_gap_skips: u64,
    pub output_late_drops: u64,
    pub output_overflow_drops: u64,
    pub output_duplicate_drops: u64,
    pub first_output_latency_ms: Option<u64>,
    pub output_max_frame_gap_ms: Option<u64>,
    pub max_playback_queue_ms: u64,
    pub vad_speech_frames: u64,
    pub vad_utterances_ended: u64,
    pub tts_audio_chunks: u64,
    pub asr_committed_chars: u64,
    pub tts_queued_jobs: u64,
    pub tts_started_jobs: u64,
    pub tts_completed_jobs: u64,
    pub tts_dropped_jobs: u64,
    pub tts_queued_chars: u64,
    pub tts_started_chars: u64,
    pub tts_completed_chars: u64,
    pub tts_dropped_chars: u64,
    pub last_event: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RealtimeFullChainVerdict {
    Pass,
    Degraded,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeFullChainTestReport {
    pub session_id: String,
    pub trace_id: String,
    pub voice_name: String,
    pub websocket_url: String,
    pub file_name: String,
    pub audio_bytes: usize,
    pub sample_rate: u32,
    pub frame_ms: u16,
    pub playback_output_mode: String,
    pub monitor_start_error: Option<String>,
    pub timeline: Vec<RealtimeFullChainTimelineSample>,
    pub summary: RealtimeFullChainSummary,
}

pub async fn run_realtime_full_chain_test(
    state: &AppState,
    request: RealtimeFullChainTestRequest,
) -> AppResult<RealtimeFullChainTestReport> {
    if request.audio_bytes.is_empty() {
        return Err(AppError::audio("full-chain realtime test audio is empty"));
    }
    let voice_name = request.voice_name.trim();
    if voice_name.is_empty() {
        return Err(AppError::realtime_session("voiceName is required"));
    }

    let mut settings = state.settings().load_or_default()?;
    if let Some(base_url) = request.backend_base_url.as_deref() {
        settings.backend.realtime.base_url = normalize_backend_base_url(base_url)?;
    }
    settings.validate().map_err(AppError::invalid_settings)?;

    let format = realtime_pcm_format(&settings);
    if settings.device.virtual_mic_enabled {
        state
            .virtual_mic()
            .set_target_device_id(settings.device.virtual_mic_device_id.clone());
    }

    let session = state.sessions().create_realtime_session(
        CreateRealtimeSessionRequest {
            voice_name: voice_name.to_string(),
            runtime_params: request.runtime_params.clone(),
            post_process_config: None,
        },
        &settings,
    )?;
    let running = state
        .sessions()
        .start_realtime_session(&session.session_id, state.audio_engine())?;

    let start_result = state
        .realtime_streams()
        .start(
            running.clone(),
            format,
            None,
            state.virtual_mic_handle(),
            settings.device.virtual_mic_enabled,
            RealtimeStreamMode::RealtimeVoice,
            true,
            true,
        )
        .await;
    if let Err(error) = start_result {
        let _ =
            state
                .sessions()
                .mark_realtime_session_failed(&running.session_id, error.to_string(), state.audio_engine());
        let _ = state.virtual_mic().stop();
        return Err(error);
    }

    let mut monitor_start_error = None;
    let should_start_monitor = request.start_monitor.unwrap_or(settings.device.monitor_enabled);
    if should_start_monitor {
        match state
            .audio_devices()
            .output_device_by_id(settings.device.output_device_id.as_deref())
        {
            Ok(output_device) => {
                if let Err(error) = state
                    .realtime_streams()
                    .start_monitor(&running.session_id, output_device)
                {
                    monitor_start_error = Some(error.to_string());
                }
            }
            Err(error) => {
                monitor_start_error = Some(error.to_string());
            }
        }
    }

    if settings.device.virtual_mic_enabled {
        state.virtual_mic().start(format)?;
    }

    if let Err(error) = state.realtime_streams().start_file_input(
        &running.session_id,
        request.file_name.clone(),
        request.audio_bytes.clone(),
    ) {
        let _ = state.realtime_streams().stop(&running.session_id).await;
        let _ = state
            .sessions()
            .stop_realtime_session(&running.session_id, state.audio_engine());
        let _ = state.virtual_mic().stop();
        return Err(error);
    }

    let timeline = collect_timeline(
        state,
        &running.session_id,
        request.poll_interval_ms.unwrap_or(DEFAULT_POLL_INTERVAL_MS),
        request.drain_grace_ms.unwrap_or(DEFAULT_DRAIN_GRACE_MS),
        request.max_duration_ms.unwrap_or(DEFAULT_MAX_DURATION_MS),
    )
    .await?;

    let _ = state.realtime_streams().stop(&running.session_id).await;
    let _ = state
        .sessions()
        .stop_realtime_session(&running.session_id, state.audio_engine());
    let _ = state.virtual_mic().stop();

    let playback_output_mode =
        playback_output_mode(&timeline, settings.device.virtual_mic_enabled, should_start_monitor);
    let summary = summarize_full_chain_timeline(&timeline, monitor_start_error.as_deref(), &playback_output_mode);

    Ok(RealtimeFullChainTestReport {
        session_id: running.session_id,
        trace_id: running.trace_id,
        voice_name: running.voice_name,
        websocket_url: running.websocket_url,
        file_name: request.file_name,
        audio_bytes: request.audio_bytes.len(),
        sample_rate: format.sample_rate,
        frame_ms: format.frame_ms,
        playback_output_mode,
        monitor_start_error,
        timeline,
        summary,
    })
}

fn realtime_pcm_format(settings: &AppSettings) -> PcmFormat {
    PcmFormat {
        sample_rate: settings.runtime.default_sample_rate,
        frame_ms: settings.runtime.audio_frame_ms,
        sample_format: SampleFormat::I16,
        ..PcmFormat::default()
    }
}

async fn collect_timeline(
    state: &AppState,
    session_id: &str,
    poll_interval_ms: u64,
    drain_grace_ms: u64,
    max_duration_ms: u64,
) -> AppResult<Vec<RealtimeFullChainTimelineSample>> {
    let started_at = Instant::now();
    let poll = Duration::from_millis(poll_interval_ms.max(100));
    let max_duration = Duration::from_millis(max_duration_ms.max(poll_interval_ms.max(100)));
    let drain_grace = Duration::from_millis(drain_grace_ms.max(poll_interval_ms.max(100)));
    let mut timeline = Vec::new();
    let mut input_completed_at: Option<Instant> = None;
    let mut last_output_progress_at = Instant::now();
    let mut last_progress_key: Option<(u64, u64, u64, u64)> = None;

    loop {
        let snapshot = state.realtime_streams().get_snapshot(session_id)?;
        let elapsed_ms = started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let progress_key = (
            snapshot.output_received_frames,
            snapshot.output_playable_frames,
            snapshot.output_written_frames,
            snapshot.monitor_frames + snapshot.virtual_mic_frames,
        );
        if last_progress_key.is_none_or(|previous| previous != progress_key) {
            last_progress_key = Some(progress_key);
            last_output_progress_at = Instant::now();
        }
        if snapshot.input_source == "local_file"
            && snapshot.input_state == "off"
            && snapshot.last_event.as_deref() == Some("file_input_completed")
            && input_completed_at.is_none()
        {
            input_completed_at = Some(Instant::now());
        }
        let terminal_error = snapshot.last_error.is_some() || snapshot.websocket_state == "error";
        timeline.push(RealtimeFullChainTimelineSample { elapsed_ms, snapshot });

        if terminal_error {
            break;
        }
        if started_at.elapsed() >= max_duration {
            break;
        }
        if let Some(done_at) = input_completed_at {
            if done_at.elapsed() >= drain_grace && last_output_progress_at.elapsed() >= drain_grace {
                break;
            }
        }
        tokio::time::sleep(poll).await;
    }

    Ok(timeline)
}

fn summarize_full_chain_timeline(
    timeline: &[RealtimeFullChainTimelineSample],
    monitor_start_error: Option<&str>,
    playback_output_mode: &str,
) -> RealtimeFullChainSummary {
    let last = timeline.last().map(|sample| &sample.snapshot);
    let mut reasons = Vec::new();
    let duration_ms = timeline.last().map(|sample| sample.elapsed_ms).unwrap_or_default();
    let max_playback_queue_ms = timeline
        .iter()
        .map(|sample| sample.snapshot.output_playback_queue_ms)
        .max()
        .unwrap_or_default();
    let output_max_frame_gap_ms = timeline
        .iter()
        .filter_map(|sample| sample.snapshot.output_max_frame_gap_ms)
        .max();

    let (
        sent_frames,
        received_frames,
        output_received_frames,
        output_playable_frames,
        output_written_frames,
        monitor_frames,
        virtual_mic_frames,
        output_ack_mismatches,
        output_gap_skips,
        output_late_drops,
        output_overflow_drops,
        output_duplicate_drops,
        first_output_latency_ms,
        vad_speech_frames,
        vad_utterances_ended,
        tts_audio_chunks,
        asr_committed_chars,
        tts_queued_jobs,
        tts_started_jobs,
        tts_completed_jobs,
        tts_dropped_jobs,
        tts_queued_chars,
        tts_started_chars,
        tts_completed_chars,
        tts_dropped_chars,
        last_event,
        last_error,
    ) = if let Some(snapshot) = last {
        (
            snapshot.sent_frames,
            snapshot.received_frames,
            snapshot.output_received_frames,
            snapshot.output_playable_frames,
            snapshot.output_written_frames,
            snapshot.monitor_frames,
            snapshot.virtual_mic_frames,
            snapshot.output_ack_mismatches,
            snapshot.output_gap_skips,
            snapshot.output_late_drops,
            snapshot.output_overflow_drops,
            snapshot.output_duplicate_drops,
            snapshot.first_output_latency_ms,
            snapshot.vad_speech_frames,
            snapshot.vad_utterances_ended,
            snapshot.tts_audio_chunks,
            snapshot.asr_committed_chars,
            snapshot.tts_queued_jobs,
            snapshot.tts_started_jobs,
            snapshot.tts_completed_jobs,
            snapshot.tts_dropped_jobs,
            snapshot.tts_queued_chars,
            snapshot.tts_started_chars,
            snapshot.tts_completed_chars,
            snapshot.tts_dropped_chars,
            snapshot.last_event.clone(),
            snapshot.last_error.clone(),
        )
    } else {
        (
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, None, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, None, None,
        )
    };

    if let Some(error) = &last_error {
        reasons.push(format!("链路错误: {error}"));
    }
    if sent_frames == 0 {
        reasons.push("未发送本地模拟音频帧".into());
    }
    if vad_speech_frames == 0 {
        reasons.push("服务端 VAD 没有确认有效语音帧".into());
    }
    if output_received_frames == 0 {
        reasons.push("未收到 FunSpeech 变声音频输出".into());
    }
    if asr_committed_chars > 0 && tts_queued_chars < asr_committed_chars {
        reasons.push(format!(
            "TTS 入队文本少于 ASR 已提交文本: queued={} committed={}",
            tts_queued_chars, asr_committed_chars
        ));
    }
    if tts_queued_chars > 0
        && (tts_completed_chars > 0 || tts_dropped_chars > 0)
        && tts_completed_chars + tts_dropped_chars < tts_queued_chars
    {
        reasons.push(format!(
            "TTS 已完成文本少于入队文本: completed={} dropped={} queued={}",
            tts_completed_chars, tts_dropped_chars, tts_queued_chars
        ));
    }
    if tts_dropped_jobs > 0 {
        reasons.push(format!("服务端 TTS 丢弃任务 {tts_dropped_jobs} 个"));
    }
    if output_playable_frames == 0 && output_received_frames > 0 {
        reasons.push("收到输出但未进入连续保序播放窗口".into());
    }
    if output_ack_mismatches > 0 {
        reasons.push(format!("输出元数据/ACK 匹配异常 {output_ack_mismatches} 次"));
    }
    let drop_count = output_gap_skips + output_late_drops + output_overflow_drops + output_duplicate_drops;
    if drop_count > 0 {
        reasons.push(format!("播放保序/背压丢弃 {drop_count} 次"));
    }
    if output_max_frame_gap_ms.is_some_and(|gap| gap > 250) {
        reasons.push(format!(
            "最大输出帧间隔 {}ms，存在可感知卡顿风险",
            output_max_frame_gap_ms.unwrap_or_default()
        ));
    }
    if max_playback_queue_ms > 2_500 {
        reasons.push(format!("最大播放预缓冲 {}ms，实时监听延迟过高", max_playback_queue_ms));
    }
    if playback_output_mode == "buffer_only" {
        reasons.push("未写入监听设备或虚拟麦克风，仅验证了输出保序缓冲与 ACK".into());
    }
    if let Some(error) = monitor_start_error {
        reasons.push(format!("监听输出启动失败: {error}"));
    }

    let verdict = if last_error.is_some() || sent_frames == 0 || output_received_frames == 0 {
        RealtimeFullChainVerdict::Fail
    } else if drop_count > 0
        || output_ack_mismatches > 0
        || (asr_committed_chars > 0 && tts_queued_chars < asr_committed_chars)
        || (tts_queued_chars > 0
            && (tts_completed_chars > 0 || tts_dropped_chars > 0)
            && tts_completed_chars + tts_dropped_chars < tts_queued_chars)
        || tts_dropped_jobs > 0
        || output_max_frame_gap_ms.is_some_and(|gap| gap > 250)
        || max_playback_queue_ms > 2_500
        || playback_output_mode == "buffer_only"
        || monitor_start_error.is_some()
    {
        RealtimeFullChainVerdict::Degraded
    } else {
        RealtimeFullChainVerdict::Pass
    };

    RealtimeFullChainSummary {
        verdict,
        reasons,
        duration_ms,
        sent_frames,
        received_frames,
        output_received_frames,
        output_playable_frames,
        output_written_frames,
        monitor_frames,
        virtual_mic_frames,
        output_ack_mismatches,
        output_gap_skips,
        output_late_drops,
        output_overflow_drops,
        output_duplicate_drops,
        first_output_latency_ms,
        output_max_frame_gap_ms,
        max_playback_queue_ms,
        vad_speech_frames,
        vad_utterances_ended,
        tts_audio_chunks,
        asr_committed_chars,
        tts_queued_jobs,
        tts_started_jobs,
        tts_completed_jobs,
        tts_dropped_jobs,
        tts_queued_chars,
        tts_started_chars,
        tts_completed_chars,
        tts_dropped_chars,
        last_event,
        last_error,
    }
}

fn playback_output_mode(
    timeline: &[RealtimeFullChainTimelineSample],
    virtual_mic_enabled: bool,
    monitor_requested: bool,
) -> String {
    let last = timeline.last().map(|sample| &sample.snapshot);
    if last.is_some_and(|snapshot| snapshot.monitor_frames > 0) {
        "monitor".into()
    } else if last.is_some_and(|snapshot| snapshot.virtual_mic_frames > 0) {
        "virtual_mic".into()
    } else if virtual_mic_enabled {
        "virtual_mic_no_frames".into()
    } else if monitor_requested {
        "monitor_no_frames".into()
    } else {
        "buffer_only".into()
    }
}

fn normalize_backend_base_url(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(AppError::invalid_settings("backend realtime baseUrl is empty"));
    }
    if let Some(rest) = trimmed.strip_prefix("ws://") {
        return Ok(format!("http://{rest}"));
    }
    if let Some(rest) = trimmed.strip_prefix("wss://") {
        return Ok(format!("https://{rest}"));
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(trimmed.to_string());
    }
    Ok(format!("http://{trimmed}"))
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_backend_base_url, summarize_full_chain_timeline, RealtimeFullChainTimelineSample,
        RealtimeFullChainVerdict,
    };
    use crate::{
        domain::{
            runtime_params::RuntimeParams,
            session::{RealtimeSession, RealtimeSessionStatus},
        },
        services::realtime_stream_manager::RealtimeStreamSnapshot,
    };

    #[test]
    fn realtime_full_chain_summary_fails_without_output_audio() {
        let snapshot = snapshot_with_counts(200, 100, 0, 0);
        let summary = summarize_full_chain_timeline(
            &[RealtimeFullChainTimelineSample {
                elapsed_ms: 1_000,
                snapshot,
            }],
            None,
            "buffer_only",
        );

        assert_eq!(summary.verdict, RealtimeFullChainVerdict::Fail);
        assert!(summary.reasons.iter().any(|reason| reason.contains("未收到 FunSpeech")));
    }

    #[test]
    fn realtime_full_chain_summary_marks_buffer_only_as_degraded() {
        let mut snapshot = snapshot_with_counts(200, 100, 50, 50);
        snapshot.vad_speech_frames = 100;
        let summary = summarize_full_chain_timeline(
            &[RealtimeFullChainTimelineSample {
                elapsed_ms: 1_000,
                snapshot,
            }],
            None,
            "buffer_only",
        );

        assert_eq!(summary.verdict, RealtimeFullChainVerdict::Degraded);
        assert!(summary.reasons.iter().any(|reason| reason.contains("ACK")));
    }

    #[test]
    fn realtime_full_chain_summary_marks_excess_playback_latency_as_degraded() {
        let mut snapshot = snapshot_with_counts(200, 100, 80, 80);
        snapshot.output_playback_queue_ms = 3_000;
        let summary = summarize_full_chain_timeline(
            &[RealtimeFullChainTimelineSample {
                elapsed_ms: 1_000,
                snapshot,
            }],
            None,
            "monitor",
        );

        assert_eq!(summary.verdict, RealtimeFullChainVerdict::Degraded);
        assert!(summary.reasons.iter().any(|reason| reason.contains("实时监听延迟过高")));
    }

    #[test]
    fn realtime_full_chain_normalizes_bare_server_address() {
        assert_eq!(
            normalize_backend_base_url("10.0.0.96:8000").unwrap(),
            "http://10.0.0.96:8000"
        );
        assert_eq!(
            normalize_backend_base_url("ws://10.0.0.96:8000").unwrap(),
            "http://10.0.0.96:8000"
        );
    }

    fn snapshot_with_counts(sent: u64, vad: u64, received: u64, playable: u64) -> RealtimeStreamSnapshot {
        let session = RealtimeSession {
            session_id: "session-1".into(),
            trace_id: "trace-1".into(),
            voice_name: "voice".into(),
            runtime_params: RuntimeParams::default(),
            post_process_config: None,
            status: RealtimeSessionStatus::Running,
            websocket_url: "ws://localhost:8000/ws/v1/realtime/voice".into(),
            error_summary: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let mut snapshot = RealtimeStreamSnapshot::pending(&session);
        snapshot.sent_frames = sent;
        snapshot.vad_speech_frames = vad;
        snapshot.output_received_frames = received;
        snapshot.output_playable_frames = playable;
        snapshot
    }
}
