use std::{path::Path, time::Duration};

use reqwest::blocking::{multipart, Client};
use serde::Deserialize;
use serde_json::json;

use crate::{
    app::error::{AppError, AppResult},
    domain::{
        settings::BackendConfig,
        voice::CustomVoiceProfile,
        voice_sync::{RemoteVoiceInfo, VoiceSyncEndpointSet},
    },
};

impl VoiceSyncEndpointSet {
    pub fn from_backend_config(config: &BackendConfig) -> Self {
        let base = config.base_url.trim_end_matches('/');
        Self {
            sync_url: format!("{base}/voices/v1/list"),
            register_url: format!("{base}/voices/v1/register"),
            update_url: format!("{base}/voices/v1/update"),
            delete_url: format!("{base}/voices/v1/delete"),
            refresh_url: format!("{base}/voices/v1/refresh"),
            timeout_ms: config.timeout_ms,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RemoteVoiceListResponse {
    #[serde(default)]
    voices: Vec<RemoteVoiceInfo>,
}

pub fn list_remote_voices(config: &BackendConfig) -> AppResult<Vec<RemoteVoiceInfo>> {
    let endpoints = VoiceSyncEndpointSet::from_backend_config(config);
    let response = authorized_client(config)?
        .get(&endpoints.sync_url)
        .send()
        .map_err(http_error)?
        .error_for_status()
        .map_err(http_error)?
        .json::<RemoteVoiceListResponse>()
        .map_err(http_error)?;
    Ok(response.voices)
}

pub fn sync_voice_asset(config: &BackendConfig, profile: &CustomVoiceProfile, overwrite: bool) -> AppResult<()> {
    let endpoints = VoiceSyncEndpointSet::from_backend_config(config);
    let endpoint = if overwrite {
        endpoints.update_url
    } else {
        endpoints.register_url
    };
    let audio_path = Path::new(&profile.reference_audio_path);
    if audio_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("wav"))
        != Some(true)
    {
        return Err(AppError::offline_job(
            "FunSpeech voice sync only accepts wav reference audio",
        ));
    }
    let audio_bytes =
        std::fs::read(audio_path).map_err(|source| AppError::io("reading custom voice wav for sync", source))?;
    let file_name = audio_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("reference.wav")
        .to_string();
    let audio_part = multipart::Part::bytes(audio_bytes)
        .file_name(file_name)
        .mime_str("audio/wav")
        .map_err(http_error)?;
    let form = multipart::Form::new()
        .text("voice_name", profile.voice_name.clone())
        .text("reference_text", profile.reference_text.clone())
        .text("voice_instruction", profile.voice_instruction.clone())
        .part("reference_audio", audio_part);

    authorized_client(config)?
        .post(endpoint)
        .multipart(form)
        .send()
        .map_err(http_error)?
        .error_for_status()
        .map_err(http_error)?;
    Ok(())
}

pub fn delete_remote_voice(config: &BackendConfig, voice_name: &str) -> AppResult<()> {
    let endpoints = VoiceSyncEndpointSet::from_backend_config(config);
    authorized_client(config)?
        .post(endpoints.delete_url)
        .json(&json!({ "voice_name": voice_name }))
        .send()
        .map_err(http_error)?
        .error_for_status()
        .map_err(http_error)?;
    Ok(())
}

pub fn refresh_remote_voices(config: &BackendConfig) -> AppResult<()> {
    let endpoints = VoiceSyncEndpointSet::from_backend_config(config);
    authorized_client(config)?
        .post(endpoints.refresh_url)
        .send()
        .map_err(http_error)?
        .error_for_status()
        .map_err(http_error)?;
    Ok(())
}

fn authorized_client(config: &BackendConfig) -> AppResult<Client> {
    Client::builder()
        .timeout(Duration::from_millis(config.timeout_ms))
        .build()
        .map_err(http_error)
}

fn http_error(error: reqwest::Error) -> AppError {
    AppError::offline_job(format!("FunSpeech voice_manager request failed: {error}"))
}

#[cfg(test)]
mod tests {
    use crate::{domain::settings::BackendConfig, domain::voice_sync::VoiceSyncEndpointSet};

    #[test]
    fn voice_manager_endpoints_map_funspeech_voice_manager_paths() {
        let mut config = BackendConfig::funspeech_default();
        config.base_url = "https://voice.example.com/".into();

        let endpoints = VoiceSyncEndpointSet::from_backend_config(&config);

        assert_eq!(endpoints.sync_url, "https://voice.example.com/voices/v1/list");
        assert_eq!(endpoints.register_url, "https://voice.example.com/voices/v1/register");
        assert_eq!(endpoints.update_url, "https://voice.example.com/voices/v1/update");
        assert_eq!(endpoints.delete_url, "https://voice.example.com/voices/v1/delete");
        assert_eq!(endpoints.refresh_url, "https://voice.example.com/voices/v1/refresh");
    }
}
