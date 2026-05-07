use serde::{Deserialize, Serialize};

use crate::domain::settings::BackendConfig;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalLlmEndpoint {
    pub provider_name: String,
    pub generate_url: String,
    pub model: Option<String>,
    pub timeout_ms: u64,
}

impl LocalLlmEndpoint {
    pub fn from_backend_config(config: &BackendConfig) -> Self {
        Self {
            provider_name: config.provider_name.clone(),
            generate_url: format!("{}/api/generate", config.base_url.trim_end_matches('/')),
            model: config.model.clone(),
            timeout_ms: config.timeout_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::settings::BackendConfig;

    use super::LocalLlmEndpoint;

    #[test]
    fn local_llm_endpoint_maps_generate_path() {
        let mut config = BackendConfig::local_llm_default();
        config.base_url = "http://127.0.0.1:11434/".into();
        config.model = Some("qwen".into());

        let endpoint = LocalLlmEndpoint::from_backend_config(&config);

        assert_eq!(endpoint.generate_url, "http://127.0.0.1:11434/api/generate");
        assert_eq!(endpoint.model.as_deref(), Some("qwen"));
    }
}
