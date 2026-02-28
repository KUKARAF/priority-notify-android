use crate::api::ApiClient;
use std::time::Duration;
use tauri::AppHandle;

/// Periodically check for unread notifications and fire a system reminder.
/// Runs until the task is aborted.
pub async fn run_reminder_loop(
    base_url: String,
    token: String,
    app: AppHandle,
    interval_minutes: u64,
) {
    let client = ApiClient::new(&base_url, &token);

    loop {
        tokio::time::sleep(Duration::from_secs(interval_minutes * 60)).await;

        match client
            .list_notifications(Some("unread"), None, None, Some(1), None)
            .await
        {
            Ok(page) if page.total > 0 => {
                let msg = if page.total == 1 {
                    "You have 1 unread notification".to_string()
                } else {
                    format!("You have {} unread notifications", page.total)
                };
                fire_reminder(&app, &msg);
            }
            Ok(_) => {
                // No unread notifications, skip
            }
            Err(e) => {
                log::error!("Reminder check failed: {e}");
            }
        }
    }
}

fn fire_reminder(app: &AppHandle, body: &str) {
    use tauri_plugin_notification::NotificationExt;

    let builder = app
        .notification()
        .builder()
        .title("Priority Notify")
        .body(body);

    if let Err(e) = builder.show() {
        log::error!("Failed to show reminder notification: {e}");
    }
}
