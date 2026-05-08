pub mod app;
pub mod audio;
pub mod clients;
pub mod domain;
pub mod services;
pub mod storage;
pub mod tauri_api;

use app::{error::AppResult, state::AppState};
use storage::app_paths::AppPaths;

fn initialize_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "voice_cloner=info,tauri=warn".into()),
        )
        .try_init();
}

pub fn build_app_state() -> AppResult<AppState> {
    AppState::new(AppPaths::discover()?)
}

pub fn run() {
    initialize_tracing();
    let state = build_app_state().expect("failed to initialize voice-cloner runtime state");

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            tauri_api::app_commands::get_app_summary,
            tauri_api::app_commands::get_app_runtime_info,
            tauri_api::settings_commands::get_app_settings,
            tauri_api::settings_commands::update_app_settings,
            tauri_api::settings_commands::reset_app_settings,
            tauri_api::settings_commands::check_backend_health,
            tauri_api::audio_commands::list_audio_input_devices,
            tauri_api::audio_commands::list_audio_output_devices,
            tauri_api::audio_commands::get_default_audio_devices,
            tauri_api::audio_commands::toggle_voice_preview,
            tauri_api::audio_commands::stop_voice_preview,
            tauri_api::realtime_commands::create_realtime_session,
            tauri_api::realtime_commands::start_realtime_session,
            tauri_api::realtime_commands::stop_realtime_session,
            tauri_api::realtime_commands::fail_realtime_session,
            tauri_api::realtime_commands::update_realtime_params,
            tauri_api::realtime_commands::switch_realtime_voice,
            tauri_api::realtime_commands::get_realtime_session,
            tauri_api::realtime_commands::list_realtime_sessions,
            tauri_api::realtime_commands::get_realtime_stream_snapshot,
            tauri_api::realtime_commands::list_realtime_stream_snapshots,
            tauri_api::offline_commands::create_offline_audio_job,
            tauri_api::offline_commands::create_offline_text_job,
            tauri_api::offline_commands::start_offline_job,
            tauri_api::offline_commands::cancel_offline_job,
            tauri_api::offline_commands::retry_offline_job,
            tauri_api::offline_commands::complete_offline_job,
            tauri_api::offline_commands::fail_offline_job,
            tauri_api::offline_commands::get_offline_job,
            tauri_api::offline_commands::list_offline_jobs,
            tauri_api::voice_design_commands::create_voice_design_draft,
            tauri_api::voice_design_commands::start_voice_design_asr,
            tauri_api::voice_design_commands::complete_voice_design_asr,
            tauri_api::voice_design_commands::start_voice_design_llm,
            tauri_api::voice_design_commands::complete_voice_instruction,
            tauri_api::voice_design_commands::start_voice_design_generation,
            tauri_api::voice_design_commands::complete_voice_design_preview,
            tauri_api::voice_design_commands::save_voice_design_draft,
            tauri_api::voice_design_commands::fail_voice_design_draft,
            tauri_api::voice_design_commands::get_voice_design_draft,
            tauri_api::voice_design_commands::list_voice_design_drafts,
            tauri_api::voice_design_commands::list_custom_voices,
            tauri_api::voice_design_commands::get_custom_voice,
            tauri_api::voice_design_commands::save_custom_voice_profile,
            tauri_api::voice_design_commands::transcribe_reference_audio,
            tauri_api::voice_sync_commands::sync_voices_full,
            tauri_api::voice_sync_commands::list_remote_voices,
            tauri_api::voice_sync_commands::register_custom_voice,
            tauri_api::voice_sync_commands::update_custom_voice_sync,
            tauri_api::voice_sync_commands::delete_custom_voice_sync,
            tauri_api::voice_sync_commands::refresh_voice_runtime,
            tauri_api::voice_sync_commands::fail_voice_sync,
            tauri_api::voice_sync_commands::list_voice_sync_reports,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
