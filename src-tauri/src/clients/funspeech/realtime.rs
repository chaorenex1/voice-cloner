use serde::{Deserialize, Serialize};

use crate::domain::settings::BackendConfig;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeEndpoint {
    pub websocket_url: String,
    pub timeout_ms: u64,
}

impl RealtimeEndpoint {
    pub fn from_backend_config(config: &BackendConfig) -> Self {
        let base = config.base_url.trim_end_matches('/');
        let websocket_base = if let Some(rest) = base.strip_prefix("https://") {
            format!("wss://{rest}")
        } else if let Some(rest) = base.strip_prefix("http://") {
            format!("ws://{rest}")
        } else {
            base.to_string()
        };

        Self {
            websocket_url: format!("{websocket_base}/ws/v1/realtime/voice"),
            timeout_ms: config.timeout_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::settings::BackendConfig;

    use super::RealtimeEndpoint;

    #[test]
    fn realtime_endpoint_maps_http_base_to_ws_path() {
        let mut config = BackendConfig::funspeech_default();
        config.base_url = "https://voice.example.com/".into();

        let endpoint = RealtimeEndpoint::from_backend_config(&config);

        assert_eq!(endpoint.websocket_url, "wss://voice.example.com/ws/v1/realtime/voice");
    }
}
