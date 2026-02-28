use crate::api::ApiClient;
use crate::models::*;
use crate::reminder;
use crate::sse;
use std::sync::Mutex;
use tauri::{AppHandle, State};
use tokio::task::JoinHandle;

/// Shared application state managed by Tauri.
pub struct AppState {
    pub base_url: Mutex<String>,
    pub token: Mutex<String>,
    pub sse_handle: Mutex<Option<JoinHandle<()>>>,
    pub reminder_handle: Mutex<Option<JoinHandle<()>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            base_url: Mutex::new(String::new()),
            token: Mutex::new(String::new()),
            sse_handle: Mutex::new(None),
            reminder_handle: Mutex::new(None),
        }
    }

    fn api_client(&self) -> Result<ApiClient, String> {
        let base_url = self.base_url.lock().unwrap();
        let token = self.token.lock().unwrap();
        if base_url.is_empty() || token.is_empty() {
            return Err("Not configured. Call configure() first.".to_string());
        }
        Ok(ApiClient::new(&base_url, &token))
    }
}

// -- Settings persistence via tauri-plugin-store --

fn save_to_store(app: &AppHandle, key: &str, value: &str) {
    use tauri_plugin_store::StoreExt;
    if let Ok(store) = app.store("settings.json") {
        store.set(key, serde_json::Value::String(value.to_string()));
    }
}

fn load_from_store(app: &AppHandle, key: &str) -> Option<String> {
    use tauri_plugin_store::StoreExt;
    if let Ok(store) = app.store("settings.json") {
        if let Some(serde_json::Value::String(s)) = store.get(key) {
            return Some(s);
        }
    }
    None
}

/// Configure the app with server URL and API token.
/// Validates the token by calling GET /api/me.
#[tauri::command]
pub async fn configure(
    app: AppHandle,
    state: State<'_, AppState>,
    server_url: String,
    token: String,
) -> Result<UserResponse, String> {
    let client = ApiClient::new(&server_url, &token);
    let user = client.get_me().await?;

    // Store in runtime state
    *state.base_url.lock().unwrap() = server_url.clone();
    *state.token.lock().unwrap() = token.clone();

    // Persist to disk
    save_to_store(&app, "server_url", &server_url);
    save_to_store(&app, "token", &token);

    Ok(user)
}

/// Load saved settings from the store. Returns (server_url, token) if available.
#[tauri::command]
pub async fn load_settings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<(String, String)>, String> {
    let server_url = load_from_store(&app, "server_url");
    let token = load_from_store(&app, "token");

    match (server_url, token) {
        (Some(url), Some(tok)) => {
            *state.base_url.lock().unwrap() = url.clone();
            *state.token.lock().unwrap() = tok.clone();
            Ok(Some((url, tok)))
        }
        _ => Ok(None),
    }
}

/// Save an arbitrary setting to the store.
#[tauri::command]
pub async fn save_setting(app: AppHandle, key: String, value: String) -> Result<(), String> {
    save_to_store(&app, &key, &value);
    Ok(())
}

/// Load an arbitrary setting from the store.
#[tauri::command]
pub async fn load_setting(app: AppHandle, key: String) -> Result<Option<String>, String> {
    Ok(load_from_store(&app, &key))
}

/// Fetch the notification list with optional filters.
#[tauri::command]
pub async fn fetch_notifications(
    state: State<'_, AppState>,
    status: Option<String>,
    priority: Option<String>,
    since: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<PaginatedNotifications, String> {
    let client = state.api_client()?;
    client
        .list_notifications(
            status.as_deref(),
            priority.as_deref(),
            since.as_deref(),
            limit,
            offset,
        )
        .await
}

/// Update a notification's status (e.g. mark as read).
#[tauri::command]
pub async fn mark_notification(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> Result<Notification, String> {
    let client = state.api_client()?;
    client.update_notification(&id, &status).await
}

/// Delete a notification.
#[tauri::command]
pub async fn delete_notification(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let client = state.api_client()?;
    client.delete_notification(&id).await
}

/// Start the SSE listener in the background.
#[tauri::command]
pub async fn start_sse(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Stop existing SSE if running
    stop_sse_inner(&state);

    let base_url = state.base_url.lock().unwrap().clone();
    let token = state.token.lock().unwrap().clone();

    if base_url.is_empty() || token.is_empty() {
        return Err("Not configured".to_string());
    }

    let handle = tokio::spawn(sse::run_sse_loop(base_url, token, app));
    *state.sse_handle.lock().unwrap() = Some(handle);

    Ok(())
}

/// Stop the SSE listener.
#[tauri::command]
pub async fn stop_sse(state: State<'_, AppState>) -> Result<(), String> {
    stop_sse_inner(&state);
    Ok(())
}

fn stop_sse_inner(state: &AppState) {
    let mut handle = state.sse_handle.lock().unwrap();
    if let Some(h) = handle.take() {
        h.abort();
    }
}

/// Start the reminder loop. Reads `reminder_interval` from the store (default 20 min).
/// If interval is 0, the reminder is disabled.
#[tauri::command]
pub async fn start_reminder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    stop_reminder_inner(&state);

    let interval: u64 = load_from_store(&app, "reminder_interval")
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    if interval == 0 {
        return Ok(());
    }

    let base_url = state.base_url.lock().unwrap().clone();
    let token = state.token.lock().unwrap().clone();

    if base_url.is_empty() || token.is_empty() {
        return Err("Not configured".to_string());
    }

    let handle = tokio::spawn(reminder::run_reminder_loop(
        base_url, token, app, interval,
    ));
    *state.reminder_handle.lock().unwrap() = Some(handle);

    Ok(())
}

/// Stop the reminder loop.
#[tauri::command]
pub async fn stop_reminder(state: State<'_, AppState>) -> Result<(), String> {
    stop_reminder_inner(&state);
    Ok(())
}

/// Update the reminder interval. Persists the setting and restarts the loop.
#[tauri::command]
pub async fn update_reminder_interval(
    app: AppHandle,
    state: State<'_, AppState>,
    minutes: u64,
) -> Result<(), String> {
    save_to_store(&app, "reminder_interval", &minutes.to_string());

    stop_reminder_inner(&state);

    if minutes == 0 {
        return Ok(());
    }

    let base_url = state.base_url.lock().unwrap().clone();
    let token = state.token.lock().unwrap().clone();

    if base_url.is_empty() || token.is_empty() {
        return Ok(());
    }

    let handle = tokio::spawn(reminder::run_reminder_loop(
        base_url, token, app, minutes,
    ));
    *state.reminder_handle.lock().unwrap() = Some(handle);

    Ok(())
}

fn stop_reminder_inner(state: &AppState) {
    let mut handle = state.reminder_handle.lock().unwrap();
    if let Some(h) = handle.take() {
        h.abort();
    }
}
