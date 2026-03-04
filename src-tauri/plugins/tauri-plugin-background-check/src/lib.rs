use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

#[cfg(mobile)]
use tauri::plugin::PluginApi;

#[cfg(mobile)]
fn init_mobile<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    let _api: PluginApi<R, ()> =
        app.plugin_api("background-check", ())?;
    Ok(())
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("background-check")
        .setup(|app, _api| {
            #[cfg(mobile)]
            init_mobile(app)?;
            Ok(())
        })
        .build()
}
