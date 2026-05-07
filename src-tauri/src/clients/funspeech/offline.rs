use serde::{Deserialize, Serialize};

use crate::domain::settings::BackendConfig;

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

fn rest_url(base_url: &str, path: &str) -> String {
    format!("{}{}", base_url.trim_end_matches('/'), path)
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
