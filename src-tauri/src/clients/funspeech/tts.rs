use std::time::Duration;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    app::error::{AppError, AppResult},
    domain::{runtime_params::RuntimeParams, settings::BackendConfig},
};

const OFFLINE_TTS_TIMEOUT_FLOOR_MS: u64 = 300_000;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeTtsEndpoint {
    pub websocket_url: String,
    pub timeout_ms: u64,
}

impl RealtimeTtsEndpoint {
    pub fn from_backend_config(config: &BackendConfig) -> Self {
        let base = config.base_url.trim_end_matches('/');
        let websocket_base = websocket_base_url(base);

        Self {
            websocket_url: format!("{websocket_base}/ws/v1/tts"),
            timeout_ms: config.timeout_ms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OfflineTtsRequest {
    pub text: String,
    pub voice_name: String,
    pub runtime_params: RuntimeParams,
    pub output_format: String,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OfflineTtsResult {
    pub audio_bytes: Vec<u8>,
    pub content_type: Option<String>,
}

pub fn synthesize_text(config: &BackendConfig, request: OfflineTtsRequest) -> AppResult<OfflineTtsResult> {
    config.validate("backend.tts").map_err(AppError::invalid_settings)?;
    let text = request.text.trim();
    if text.is_empty() {
        return Err(AppError::offline_job("text is required for FunSpeech TTS"));
    }
    if request.voice_name.trim().is_empty() {
        return Err(AppError::offline_job("voiceName is required for FunSpeech TTS"));
    }

    let sample_rate = funspeech_tts_sample_rate(request.sample_rate);
    let mut result = send_tts_request(config, &request, text, sample_rate)?;
    if request.output_format.eq_ignore_ascii_case("wav")
        && sample_rate != FUNSPEECH_PRESET_SAMPLE_RATE
        && looks_like_tiny_wav(&result.audio_bytes)
    {
        result = send_tts_request(config, &request, text, FUNSPEECH_PRESET_SAMPLE_RATE)?;
    }

    Ok(result)
}

fn send_tts_request(
    config: &BackendConfig,
    request: &OfflineTtsRequest,
    text: &str,
    sample_rate: u32,
) -> AppResult<OfflineTtsResult> {
    let endpoint = rest_url(&config.base_url, "/stream/v1/tts");
    let response = Client::builder()
        .timeout(Duration::from_millis(offline_tts_timeout_ms(config)))
        .build()
        .map_err(tts_http_error)?
        .post(endpoint)
        .json(&json!({
            "text": text,
            "voice": request.voice_name,
            "format": request.output_format,
            "sample_rate": sample_rate,
            "speech_rate": json_number(&request.runtime_params, "speechRate")
                .or_else(|| json_number(&request.runtime_params, "speech_rate"))
                .unwrap_or(0.0),
            "volume": json_number(&request.runtime_params, "volume").unwrap_or(50.0) as i64,
            "pitch_rate": json_number(&request.runtime_params, "pitchRate")
                .or_else(|| json_number(&request.runtime_params, "pitch_rate"))
                .or_else(|| json_number(&request.runtime_params, "pitch"))
                .unwrap_or(0.0) as i64,
            "prompt": json_string(&request.runtime_params, "prompt").unwrap_or_default(),
        }))
        .send()
        .map_err(tts_http_error)?
        .error_for_status()
        .map_err(tts_http_error)?;

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let audio_bytes = response.bytes().map_err(tts_http_error)?.to_vec();
    if audio_bytes.is_empty() {
        return Err(AppError::offline_job("FunSpeech TTS returned empty audio"));
    }

    Ok(OfflineTtsResult {
        audio_bytes,
        content_type,
    })
}

fn rest_url(base_url: &str, path: &str) -> String {
    format!("{}{}", base_url.trim_end_matches('/'), path)
}

const FUNSPEECH_PRESET_SAMPLE_RATE: u32 = 22_050;

fn funspeech_tts_sample_rate(sample_rate: u32) -> u32 {
    const SUPPORTED: [u32; 4] = [8_000, 16_000, 22_050, 24_000];
    if sample_rate > 24_000 {
        return FUNSPEECH_PRESET_SAMPLE_RATE;
    }
    SUPPORTED
        .into_iter()
        .min_by_key(|supported| supported.abs_diff(sample_rate))
        .unwrap_or(FUNSPEECH_PRESET_SAMPLE_RATE)
}

fn looks_like_tiny_wav(audio_bytes: &[u8]) -> bool {
    audio_bytes.len() >= 12
        && &audio_bytes[0..4] == b"RIFF"
        && &audio_bytes[8..12] == b"WAVE"
        && audio_bytes.len() < 2048
}

fn offline_tts_timeout_ms(config: &BackendConfig) -> u64 {
    config.timeout_ms.max(OFFLINE_TTS_TIMEOUT_FLOOR_MS)
}

fn json_number(params: &RuntimeParams, key: &str) -> Option<f64> {
    params.values.get(key).and_then(serde_json::Value::as_f64)
}

fn json_string(params: &RuntimeParams, key: &str) -> Option<String> {
    params
        .values
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn websocket_base_url(base: &str) -> String {
    if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        base.to_string()
    }
}

fn tts_http_error(error: reqwest::Error) -> AppError {
    let mut message = format!("FunSpeech TTS request failed: {error}");
    let mut source = std::error::Error::source(&error);
    while let Some(error) = source {
        message.push_str(&format!("; caused by: {error}"));
        source = error.source();
    }
    AppError::offline_job(message)
}

#[cfg(test)]
mod tests {
    use crate::domain::settings::BackendConfig;

    use super::{funspeech_tts_sample_rate, offline_tts_timeout_ms, RealtimeTtsEndpoint};

    #[test]
    fn realtime_tts_endpoint_maps_http_base_to_ws_path() {
        let mut config = BackendConfig::funspeech_default();
        config.base_url = "https://voice.example.com/".into();

        let endpoint = RealtimeTtsEndpoint::from_backend_config(&config);

        assert_eq!(endpoint.websocket_url, "wss://voice.example.com/ws/v1/tts");
    }

    #[test]
    fn funspeech_tts_sample_rate_uses_supported_nearest_rate() {
        assert_eq!(funspeech_tts_sample_rate(48_000), 22_050);
        assert_eq!(funspeech_tts_sample_rate(24_000), 24_000);
        assert_eq!(funspeech_tts_sample_rate(16_000), 16_000);
    }

    #[test]
    fn tiny_wav_detection_catches_empty_builtin_voice_outputs() {
        let tiny_wav = b"RIFFJ\0\0\0WAVEfmt ";

        assert!(super::looks_like_tiny_wav(tiny_wav));
        assert!(!super::looks_like_tiny_wav(&vec![1; 4096]));
    }

    #[test]
    fn offline_tts_timeout_uses_long_running_floor() {
        let mut config = BackendConfig::funspeech_default();
        config.timeout_ms = 30_000;

        assert_eq!(offline_tts_timeout_ms(&config), 300_000);

        config.timeout_ms = 600_000;
        assert_eq!(offline_tts_timeout_ms(&config), 600_000);
    }
}
