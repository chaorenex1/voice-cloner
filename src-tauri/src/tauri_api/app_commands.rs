use serde::Serialize;
use tauri::State;

use crate::{
    app::{error::ApiResult, state::AppState, trace::TraceId},
    storage::app_paths::AppPaths,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSummary {
    pub name: &'static str,
    pub version: &'static str,
    pub status: &'static str,
    pub message: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppRuntimeInfo {
    pub summary: AppSummary,
    pub trace_id: String,
    pub paths: AppPaths,
}

pub fn app_summary() -> AppSummary {
    AppSummary {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        status: "ready",
        message: "Voice Cloner runtime foundation is ready for feature development.",
    }
}

#[tauri::command]
pub fn get_app_summary() -> AppSummary {
    app_summary()
}

#[tauri::command]
pub fn get_app_runtime_info(state: State<'_, AppState>) -> ApiResult<AppRuntimeInfo> {
    Ok(AppRuntimeInfo {
        summary: app_summary(),
        trace_id: TraceId::new("app").as_str().to_string(),
        paths: state.paths().clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::app_summary;

    #[test]
    fn app_summary_exposes_runtime_foundation_status() {
        let summary = app_summary();

        assert_eq!(summary.name, "voice-cloner");
        assert_eq!(summary.status, "ready");
        assert!(summary.message.contains("runtime foundation"));
    }
}
