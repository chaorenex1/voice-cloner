use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct AppSummary {
    name: &'static str,
    version: &'static str,
    status: &'static str,
    message: &'static str,
}

fn app_summary() -> AppSummary {
    AppSummary {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        status: "ready",
        message: "Voice Cloner desktop skeleton is ready for feature development.",
    }
}

#[tauri::command]
fn get_app_summary() -> AppSummary {
    app_summary()
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_app_summary])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::app_summary;

    #[test]
    fn app_summary_exposes_voice_cloner_identity() {
        let summary = app_summary();

        assert_eq!(summary.name, "voice-cloner");
        assert_eq!(summary.status, "ready");
        assert!(summary.message.contains("Voice Cloner"));
    }
}
