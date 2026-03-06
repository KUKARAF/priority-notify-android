const COMMANDS: &[&str] = &[
    "get_distributors",
    "register",
    "unregister",
    "get_push_status",
    "save_credentials",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
