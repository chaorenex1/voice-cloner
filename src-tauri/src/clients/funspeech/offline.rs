use std::{thread, time::Duration};

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
    pub async_asr_url: String,
    pub tts_url: String,
    pub openai_tts_url: String,
    pub timeout_ms: u64,
}

impl OfflineEndpoints {
    pub fn from_backend_configs(asr: &BackendConfig, tts: &BackendConfig) -> Self {
        Self {
            asr_url: rest_url(&asr.base_url, "/stream/v1/asr"),
            async_asr_url: rest_url(&asr.base_url, "/rest/v1/asr/async"),
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

#[derive(Debug, Serialize)]
struct AsyncAsrSubmitRequest<'a> {
    payload: AsyncAsrPayload<'a>,
    header: AsyncAsrHeader,
}

#[derive(Debug, Serialize)]
struct AsyncAsrPayload<'a> {
    asr_request: AsyncAsrRequestData<'a>,
    enable_notify: bool,
}

#[derive(Debug, Serialize)]
struct AsyncAsrRequestData<'a> {
    audio_bytes: &'a [u8],
    format: &'a str,
    sample_rate: u32,
    enable_punctuation_prediction: bool,
    enable_inverse_text_normalization: bool,
    enable_voice_detection: bool,
}

#[derive(Debug, Clone, Serialize)]
struct AsyncAsrHeader {
    appkey: String,
    token: String,
}

#[derive(Debug, Deserialize)]
struct AsyncAsrResponse {
    error_code: i64,
    error_message: String,
    data: Option<AsyncAsrTaskData>,
}

#[derive(Debug, Deserialize)]
struct AsyncAsrTaskData {
    task_id: String,
    result: Option<String>,
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

pub fn transcribe_audio_bytes_async(config: &BackendConfig, audio_bytes: &[u8], format: &str) -> AppResult<String> {
    config.validate("backend.asr").map_err(AppError::invalid_settings)?;
    if audio_bytes.is_empty() {
        return Err(AppError::offline_job("audio bytes are required for async ASR"));
    }
    let format = normalize_audio_format(format)?;
    let endpoint = rest_url(&config.base_url, "/rest/v1/asr/async");
    let client = Client::builder()
        .timeout(Duration::from_millis(config.timeout_ms.max(1)))
        .build()
        .map_err(http_error)?;
    let auth = async_asr_header(config);
    let submit = AsyncAsrSubmitRequest {
        payload: AsyncAsrPayload {
            asr_request: AsyncAsrRequestData {
                audio_bytes,
                format: &format,
                sample_rate: 16_000,
                enable_punctuation_prediction: true,
                enable_inverse_text_normalization: true,
                enable_voice_detection: true,
            },
            enable_notify: false,
        },
        header: auth.clone(),
    };
    let submitted = client
        .post(&endpoint)
        .json(&submit)
        .send()
        .map_err(http_error)?
        .error_for_status()
        .map_err(http_error)?
        .json::<AsyncAsrResponse>()
        .map_err(http_error)?;
    let task_id = submitted
        .data
        .as_ref()
        .ok_or_else(|| async_asr_error(&submitted))?
        .task_id
        .clone();

    poll_async_asr(&client, &endpoint, &auth, &task_id, config.timeout_ms)
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

fn async_asr_header(config: &BackendConfig) -> AsyncAsrHeader {
    AsyncAsrHeader {
        appkey: config
            .extra_options
            .get("appkey")
            .cloned()
            .unwrap_or_else(|| "voice-cloner".into()),
        token: config
            .api_key_ref
            .clone()
            .unwrap_or_else(|| "voice-cloner-token".into()),
    }
}

fn poll_async_asr(
    client: &Client,
    endpoint: &str,
    auth: &AsyncAsrHeader,
    task_id: &str,
    timeout_ms: u64,
) -> AppResult<String> {
    let poll_count = ((timeout_ms.max(120_000) / 1_000).max(1)) as usize;
    for _ in 0..poll_count {
        let response = client
            .get(endpoint)
            .query(&[
                ("appkey", auth.appkey.as_str()),
                ("token", auth.token.as_str()),
                ("task_id", task_id),
            ])
            .send()
            .map_err(http_error)?
            .error_for_status()
            .map_err(http_error)?
            .json::<AsyncAsrResponse>()
            .map_err(http_error)?;
        if let Some(data) = response.data.as_ref() {
            if let Some(result) = data.result.as_ref() {
                let text = result.trim().to_string();
                if !text.is_empty() {
                    return Ok(text);
                }
            }
        }
        if response.error_message != "RUNNING" && response.error_code != 20000000 {
            return Err(async_asr_error(&response));
        }
        thread::sleep(Duration::from_secs(1));
    }
    Err(AppError::offline_job(format!(
        "FunSpeech async ASR timed out waiting for task {task_id}"
    )))
}

fn async_asr_error(response: &AsyncAsrResponse) -> AppError {
    AppError::offline_job(format!(
        "FunSpeech async ASR failed: {} ({})",
        response.error_message, response.error_code
    ))
}

#[cfg(test)]
mod tests {
    use crate::domain::settings::BackendConfig;

    use super::{async_asr_header, OfflineEndpoints};

    #[test]
    fn offline_endpoints_map_funspeech_rest_paths() {
        let mut config = BackendConfig::funspeech_default();
        config.base_url = "https://voice.example.com/".into();

        let endpoints = OfflineEndpoints::from_backend_configs(&config, &config);

        assert_eq!(endpoints.asr_url, "https://voice.example.com/stream/v1/asr");
        assert_eq!(endpoints.async_asr_url, "https://voice.example.com/rest/v1/asr/async");
        assert_eq!(endpoints.tts_url, "https://voice.example.com/stream/v1/tts");
        assert_eq!(
            endpoints.openai_tts_url,
            "https://voice.example.com/openai/v1/audio/speech"
        );
    }

    #[test]
    fn async_asr_header_uses_optional_appkey_and_token_settings() {
        let mut config = BackendConfig::funspeech_default();
        config.api_key_ref = Some("0123456789".into());
        config.extra_options.insert("appkey".into(), "desktop".into());

        let header = async_asr_header(&config);

        assert_eq!(header.appkey, "desktop");
        assert_eq!(header.token, "0123456789");
    }
}
