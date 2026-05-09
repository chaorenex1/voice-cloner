use std::time::Duration;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::{
    app::error::{AppError, AppResult},
    domain::settings::BackendConfig,
};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OfflineEndpoints {
    pub asr_url: String,
    pub tts_url: String,
    pub openai_tts_url: String,
    pub timeout_ms: u64,
}

impl OfflineEndpoints {
    pub fn from_backend_configs(asr: &BackendConfig, tts: &BackendConfig) -> Self {
        Self {
            asr_url: rest_url(&asr.base_url, "/stream/v1/asr"),
            tts_url: rest_url(&tts.base_url, "/stream/v1/tts"),
            openai_tts_url: rest_url(&tts.base_url, "/openai/v1/audio/speech"),
            timeout_ms: tts.timeout_ms.max(asr.timeout_ms),
        }
    }
}

#[derive(Debug, Deserialize)]
struct AsrTranscriptionResponse {
    result: Option<String>,
    text: Option<String>,
    message: Option<String>,
}

pub fn transcribe_wav_bytes(config: &BackendConfig, wav_bytes: &[u8]) -> AppResult<String> {
    transcribe_audio_bytes(config, wav_bytes, "wav")
}

pub fn transcribe_audio_bytes(config: &BackendConfig, audio_bytes: &[u8], format: &str) -> AppResult<String> {
    config.validate("backend.asr").map_err(AppError::invalid_settings)?;
    if audio_bytes.is_empty() {
        return Err(AppError::offline_job("audio bytes are required for ASR"));
    }
    let format = normalize_audio_format(format)?;

    let endpoint = rest_url(&config.base_url, "/stream/v1/asr");
    let response = Client::builder()
        .timeout(Duration::from_millis(config.timeout_ms.max(1)))
        .build()
        .map_err(http_error)?
        .post(endpoint)
        .query(&[
            ("format", format.as_str()),
            ("sample_rate", "16000"),
            ("enable_punctuation_prediction", "true"),
            ("enable_inverse_text_normalization", "true"),
            ("enable_voice_detection", "true"),
        ])
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .body(audio_bytes.to_vec())
        .send()
        .map_err(http_error)?
        .error_for_status()
        .map_err(http_error)?
        .json::<AsrTranscriptionResponse>()
        .map_err(http_error)?;

    let text = response.text.or(response.result).unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Err(AppError::offline_job(
            response
                .message
                .unwrap_or_else(|| "FunSpeech ASR returned an empty transcription".into()),
        ));
    }
    Ok(text)
}

fn normalize_audio_format(format: &str) -> AppResult<String> {
    let normalized = format.trim().trim_start_matches('.').to_ascii_lowercase();
    match normalized.as_str() {
        "wav" | "mp3" | "m4a" => Ok(normalized),
        _ => Err(AppError::offline_job("audio input must be wav, mp3, or m4a")),
    }
}

fn rest_url(base_url: &str, path: &str) -> String {
    format!("{}{}", base_url.trim_end_matches('/'), path)
}

fn http_error(error: reqwest::Error) -> AppError {
    AppError::offline_job(format!("FunSpeech ASR request failed: {error}"))
}

#[cfg(test)]
mod tests {
    use crate::domain::settings::BackendConfig;

    use super::OfflineEndpoints;

    #[test]
    fn offline_endpoints_map_funspeech_rest_paths() {
        let mut config = BackendConfig::funspeech_default();
        config.base_url = "https://voice.example.com/".into();

        let endpoints = OfflineEndpoints::from_backend_configs(&config, &config);

        assert_eq!(endpoints.asr_url, "https://voice.example.com/stream/v1/asr");
        assert_eq!(endpoints.tts_url, "https://voice.example.com/stream/v1/tts");
        assert_eq!(
            endpoints.openai_tts_url,
            "https://voice.example.com/openai/v1/audio/speech"
        );
    }
}
