use crate::domain::{settings::BackendConfig, voice_sync::VoiceSyncEndpointSet};

impl VoiceSyncEndpointSet {
    pub fn from_backend_config(config: &BackendConfig) -> Self {
        let base = config.base_url.trim_end_matches('/');
        Self {
            sync_url: format!("{base}/voices/v1/sync"),
            register_url: format!("{base}/voices/v1/register"),
            update_url: format!("{base}/voices/v1/update"),
            delete_url: format!("{base}/voices/v1/delete"),
            refresh_url: format!("{base}/voices/v1/refresh"),
            timeout_ms: config.timeout_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{domain::settings::BackendConfig, domain::voice_sync::VoiceSyncEndpointSet};

    #[test]
    fn voice_manager_endpoints_map_funspeech_voice_manager_paths() {
        let mut config = BackendConfig::funspeech_default();
        config.base_url = "https://voice.example.com/".into();

        let endpoints = VoiceSyncEndpointSet::from_backend_config(&config);

        assert_eq!(endpoints.sync_url, "https://voice.example.com/voices/v1/sync");
        assert_eq!(endpoints.register_url, "https://voice.example.com/voices/v1/register");
        assert_eq!(endpoints.update_url, "https://voice.example.com/voices/v1/update");
        assert_eq!(endpoints.delete_url, "https://voice.example.com/voices/v1/delete");
        assert_eq!(endpoints.refresh_url, "https://voice.example.com/voices/v1/refresh");
    }
}
