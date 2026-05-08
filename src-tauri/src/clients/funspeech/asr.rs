use serde::{Deserialize, Serialize};

use crate::domain::settings::BackendConfig;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RealtimeAsrEndpoint {
    pub websocket_url: String,
    pub timeout_ms: u64,
}

impl RealtimeAsrEndpoint {
    pub fn from_backend_config(config: &BackendConfig) -> Self {
        let base = config.base_url.trim_end_matches('/');
        let websocket_base = websocket_base_url(base);

        Self {
            websocket_url: format!("{websocket_base}/ws/v1/asr"),
            timeout_ms: config.timeout_ms,
        }
    }
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

#[cfg(test)]
mod tests {
    use crate::domain::settings::BackendConfig;

    use super::RealtimeAsrEndpoint;

    #[test]
    fn realtime_asr_endpoint_maps_http_base_to_ws_path() {
        let mut config = BackendConfig::funspeech_default();
        config.base_url = "https://voice.example.com/".into();

        let endpoint = RealtimeAsrEndpoint::from_backend_config(&config);

        assert_eq!(endpoint.websocket_url, "wss://voice.example.com/ws/v1/asr");
    }
}
