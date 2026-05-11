use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    app::{
        error::{ApiError, ApiResult},
        state::AppState,
    },
    domain::settings::{AppSettings, AppSettingsPatch, BackendConfig},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendHealthRequest {
    pub services: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackendHealthSnapshot {
    pub service: String,
    pub status: String,
    pub latency_ms: Option<u64>,
    pub message: String,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackendHealthResult {
    pub health: Vec<BackendHealthSnapshot>,
}

#[tauri::command]
pub fn get_app_settings(state: State<'_, AppState>) -> ApiResult<AppSettings> {
    state.settings().load_or_default().map_err(Into::into)
}

#[tauri::command]
pub fn update_app_settings(state: State<'_, AppState>, patch: AppSettingsPatch) -> ApiResult<AppSettings> {
    let current = state.settings().load_or_default().map_err(ApiError::from)?;
    let saved = state
        .settings()
        .replace_validated(patch.apply_to(current.clone()))
        .map_err(ApiError::from)?;
    if let Err(error) = state.mcp_server().apply_settings(&saved.backend.mcp) {
        let _ = state.settings().replace_validated(current.clone());
        let _ = state.mcp_server().apply_settings(&current.backend.mcp);
        return Err(ApiError::from(error));
    }
    state
        .virtual_mic()
        .set_target_device_id(saved.device.virtual_mic_device_id.clone());
    Ok(saved)
}

#[tauri::command]
pub fn reset_app_settings(state: State<'_, AppState>) -> ApiResult<AppSettings> {
    let settings = state.settings().reset().map_err(ApiError::from)?;
    state
        .mcp_server()
        .apply_settings(&settings.backend.mcp)
        .map_err(ApiError::from)?;
    state
        .virtual_mic()
        .set_target_device_id(settings.device.virtual_mic_device_id.clone());
    Ok(settings)
}

#[tauri::command]
pub async fn check_backend_health(
    state: State<'_, AppState>,
    request: BackendHealthRequest,
) -> ApiResult<BackendHealthResult> {
    let settings = state.settings().load_or_default().map_err(ApiError::from)?;
    let mut health = Vec::with_capacity(request.services.len());

    for service in request.services {
        let checked_at = Utc::now().to_rfc3339();
        let Some(config) = backend_config_for_service(&settings, &service) else {
            health.push(BackendHealthSnapshot {
                service,
                status: "warning".into(),
                latency_ms: None,
                message: "unknown backend service".into(),
                checked_at,
            });
            continue;
        };

        health.push(check_backend_endpoint(service, config, checked_at).await);
    }

    Ok(BackendHealthResult { health })
}

fn backend_config_for_service<'a>(settings: &'a AppSettings, service: &str) -> Option<&'a BackendConfig> {
    match service {
        "llm" => Some(&settings.backend.llm),
        "asr" => Some(&settings.backend.asr),
        "tts" => Some(&settings.backend.tts),
        "realtime" => Some(&settings.backend.realtime),
        _ => None,
    }
}

async fn check_backend_endpoint(service: String, config: &BackendConfig, checked_at: String) -> BackendHealthSnapshot {
    let started = Instant::now();
    let timeout_ms = config.timeout_ms.max(1);
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return BackendHealthSnapshot {
                service,
                status: "error".into(),
                latency_ms: None,
                message: format!("failed to create HTTP client: {error}"),
                checked_at,
            }
        }
    };

    match client.get(config.base_url.trim_end_matches('/')).send().await {
        Ok(response) => {
            let elapsed = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
            let status = response.status();
            let is_success = status.is_success();
            BackendHealthSnapshot {
                service,
                status: if is_success { "ok" } else { "warning" }.into(),
                latency_ms: Some(elapsed),
                message: format!("HTTP {status} from {}", config.base_url),
                checked_at,
            }
        }
        Err(error) => BackendHealthSnapshot {
            service,
            status: "error".into(),
            latency_ms: Some(started.elapsed().as_millis().min(u64::MAX as u128) as u64),
            message: error.to_string(),
            checked_at,
        },
    }
}
