use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    App, AppHandle, Manager, Runtime, WebviewWindow, WindowEvent,
};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_SHOW_ID: &str = "show-main";
const TRAY_QUIT_ID: &str = "quit";

pub fn setup(app: &mut App) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, TRAY_SHOW_ID, "显示主窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, TRAY_QUIT_ID, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
    let icon = app.default_window_icon().cloned();

    let mut tray = TrayIconBuilder::new()
        .tooltip("voice-cloner")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_SHOW_ID => show_main_window(app),
            TRAY_QUIT_ID => app.exit(0),
            _ => {}
        });
    if let Some(icon) = icon {
        tray = tray.icon(icon);
    }
    tray.build(app)?;

    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        install_close_to_tray(window);
    }

    Ok(())
}

fn install_close_to_tray<R: Runtime>(window: WebviewWindow<R>) {
    window.on_window_event({
        let window = window.clone();
        move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(error) = window.hide() {
                    tracing::warn!(%error, "failed to hide main window to tray");
                }
            }
        }
    });
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        if let Err(error) = window.show() {
            tracing::warn!(%error, "failed to show main window from tray");
        }
        if let Err(error) = window.unminimize() {
            tracing::debug!(%error, "main window was not minimized");
        }
        if let Err(error) = window.set_focus() {
            tracing::debug!(%error, "failed to focus main window");
        }
    }
}
