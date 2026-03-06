use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("unifiedpush")
        .setup(|_app, _api| {
            Ok(())
        })
        .build()
}
