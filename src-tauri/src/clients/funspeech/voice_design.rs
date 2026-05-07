use serde::{Deserialize, Serialize};

use crate::domain::settings::BackendConfig;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDesignEndpoint {
    pub provider_name: String,
    pub voice_design_url: String,
    pub timeout_ms: u64,
}

impl VoiceDesignEndpoint {
    pub fn from_backend_config(config: &BackendConfig) -> Self {
        Self {
            provider_name: config.provider_name.clone(),
            voice_design_url: format!("{}/voices/v1/voice-design", config.base_url.trim_end_matches('/')),
            timeout_ms: config.timeout_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::settings::BackendConfig;

    use super::VoiceDesignEndpoint;

    #[test]
    fn voice_design_endpoint_maps_funspeech_voice_design_path() {
        let mut config = BackendConfig::funspeech_default();
        config.base_url = "https://voice.example.com/".into();

        let endpoint = VoiceDesignEndpoint::from_backend_config(&config);

        assert_eq!(
            endpoint.voice_design_url,
            "https://voice.example.com/voices/v1/voice-design"
        );
        assert_eq!(endpoint.provider_name, "funspeech");
    }
}
