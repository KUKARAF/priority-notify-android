mod api;
mod commands;
mod models;
mod sse;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build());

    #[cfg(mobile)]
    {
        builder = builder.plugin(tauri_plugin_barcode_scanner::init());
        builder = builder.plugin(tauri_plugin_unifiedpush::init());
    }

    builder
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::configure,
            commands::load_settings,
            commands::save_setting,
            commands::load_setting,
            commands::fetch_notifications,
            commands::mark_notification,
            commands::delete_notification,
            commands::start_sse,
            commands::stop_sse,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
