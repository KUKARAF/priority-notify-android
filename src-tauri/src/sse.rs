use crate::models::{Notification, Priority, StatusChangeEvent};
use reqwest::Client;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Parse a single SSE field line into (field_name, value).
fn parse_sse_line(line: &str) -> Option<(&str, &str)> {
    let colon = line.find(':')?;
    let field = &line[..colon];
    let value = line[colon + 1..].strip_prefix(' ').unwrap_or(&line[colon + 1..]);
    Some((field, value))
}

/// Connect to the SSE endpoint and process events until disconnect.
async fn connect_and_listen(
    client: &Client,
    base_url: &str,
    token: &str,
    last_event_id: &mut Option<String>,
    app: &AppHandle,
) -> Result<(), String> {
    let url = format!("{base_url}/api/notifications/stream");

    let mut req = client
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "text/event-stream");

    if let Some(ref id) = last_event_id {
        req = req.header("Last-Event-ID", id);
    }

    let response = req.send().await.map_err(|e| format!("SSE connect: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("SSE status: {}", response.status()));
    }

    app.emit("sse-status", "connected").ok();

    let mut buffer = String::new();
    let mut event_type = String::new();
    let mut data = String::new();
    let mut event_id = String::new();

    use futures::StreamExt;

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("SSE read: {e}"))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                // End of event — dispatch it
                if !event_type.is_empty() || !data.is_empty() {
                    dispatch_event(&event_type, &data, app);
                }
                if !event_id.is_empty() {
                    *last_event_id = Some(event_id.clone());
                }
                event_type.clear();
                data.clear();
                event_id.clear();
            } else if let Some((field, value)) = parse_sse_line(&line) {
                match field {
                    "event" => event_type = value.to_string(),
                    "data" => {
                        if !data.is_empty() {
                            data.push('\n');
                        }
                        data.push_str(value);
                    }
                    "id" => event_id = value.to_string(),
                    _ => {} // ignore retry, comments, etc.
                }
            }
        }
    }

    Err("SSE stream ended".to_string())
}

/// Dispatch a parsed SSE event to the frontend.
fn dispatch_event(event_type: &str, data: &str, app: &AppHandle) {
    match event_type {
        "notification" => {
            if let Ok(notif) = serde_json::from_str::<Notification>(data) {
                // Fire Android system notification for high/critical
                if matches!(notif.priority, Priority::High | Priority::Critical) {
                    fire_system_notification(&notif, app);
                }
                app.emit("new-notification", &notif).ok();
            }
        }
        "status_change" => {
            if let Ok(evt) = serde_json::from_str::<StatusChangeEvent>(data) {
                app.emit("status-change", &evt).ok();
            }
        }
        "ping" => {
            // keepalive, ignore
        }
        _ => {}
    }
}

/// Fire an Android system notification via tauri-plugin-notification.
fn fire_system_notification(notif: &Notification, app: &AppHandle) {
    use tauri_plugin_notification::NotificationExt;

    let mut builder = app.notification().builder();
    builder = builder
        .title(&notif.title)
        .body(notif.message.as_deref().unwrap_or(""));

    if let Err(e) = builder.show() {
        log::error!("Failed to show system notification: {e}");
    }
}

/// Poll for new notifications as a fallback when SSE fails repeatedly.
async fn poll_fallback(
    client: &Client,
    base_url: &str,
    token: &str,
    last_seen: &mut Option<String>,
    app: &AppHandle,
) {
    let mut url = format!("{base_url}/api/notifications/?limit=50");
    if let Some(ref since) = last_seen {
        url.push_str(&format!("&since={since}"));
    }

    let resp = match client.get(&url).bearer_auth(token).send().await {
        Ok(r) => r,
        Err(e) => {
            log::error!("Poll error: {e}");
            return;
        }
    };

    if !resp.status().is_success() {
        return;
    }

    if let Ok(page) = resp
        .json::<crate::models::PaginatedNotifications>()
        .await
    {
        for notif in &page.items {
            if matches!(notif.priority, Priority::High | Priority::Critical) {
                fire_system_notification(notif, app);
            }
            app.emit("new-notification", notif).ok();
        }
        // Track the most recent timestamp
        if let Some(newest) = page.items.first() {
            *last_seen = Some(newest.created_at.clone());
        }
    }
}

/// Main SSE loop with reconnection and polling fallback.
/// Runs until the task is aborted.
pub async fn run_sse_loop(base_url: String, token: String, app: AppHandle) {
    let client = Client::new();
    let mut backoff = Duration::from_secs(1);
    let mut consecutive_failures: u32 = 0;
    let mut last_event_id: Option<String> = None;
    let mut last_seen: Option<String> = None;

    loop {
        match connect_and_listen(&client, &base_url, &token, &mut last_event_id, &app).await {
            Ok(()) => {
                // Clean disconnect (shouldn't normally happen)
                backoff = Duration::from_secs(1);
                consecutive_failures = 0;
            }
            Err(e) => {
                consecutive_failures += 1;
                log::error!("SSE failure #{consecutive_failures}: {e}");

                if consecutive_failures > 5 {
                    // Switch to polling mode
                    app.emit("sse-status", "polling").ok();

                    // Poll every 30s for 5 minutes, then retry SSE
                    for _ in 0..10 {
                        tokio::time::sleep(Duration::from_secs(30)).await;
                        poll_fallback(&client, &base_url, &token, &mut last_seen, &app).await;
                    }

                    consecutive_failures = 0;
                    backoff = Duration::from_secs(1);
                    continue;
                }

                app.emit(
                    "sse-status",
                    serde_json::json!({
                        "status": "reconnecting",
                        "delay_secs": backoff.as_secs()
                    }),
                )
                .ok();

                tokio::time::sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, Duration::from_secs(30));
            }
        }
    }
}
