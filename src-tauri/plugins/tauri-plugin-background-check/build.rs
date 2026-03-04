const COMMANDS: &[&str] = &["schedule", "cancel", "update_settings"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
