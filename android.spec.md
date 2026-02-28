# priority-notify — Android Client

## Overview
An Android notification client built with Tauri 2.x Mobile. The app connects to the priority-notify server, displays notifications, and pushes high-priority alerts as Android system notifications. Built entirely from Rust + HTML/CSS/JS, compiled to APK via GitHub Actions.

## Tech Stack

- **Framework**: Tauri 2.x (mobile support)
- **Backend logic**: Rust (Tauri commands for SSE, token storage, notification scheduling)
- **Frontend UI**: HTML + CSS + vanilla JS (rendered in Android WebView)
- **Build**: Cargo + Gradle (Tauri handles the bridge)
- **Minimum Android version**: API 26 (Android 8.0) — required for notification channels

### Key Rust Crates
- `tauri` (with mobile feature flags)
- `reqwest` — HTTP client for API calls and SSE
- `serde` + `serde_json` — serialization
- `keyring` or `tauri-plugin-store` — secure token storage
- `tokio` — async runtime for background SSE listener

### Tauri Plugins
- `tauri-plugin-notification` — Android system notifications
- `tauri-plugin-barcode-scanner` — QR code scanning for token setup
- `tauri-plugin-store` — Persistent key-value storage (server URL, token)

## Features

### Token Setup
1. User opens the app, enters the server URL
2. Scans a QR code (from the web UI) or pastes the API token manually
3. Token is stored securely on-device via `tauri-plugin-store`
4. App validates the token by calling `GET /api/me`

### Notification List
- Fetches notifications from `GET /api/notifications/`
- Displays as a scrollable list, newest first
- Color-coded by priority (low=gray, medium=blue, high=orange, critical=red)
- Tap to expand full message body
- Swipe or button to mark as read (`PATCH /api/notifications/{id}`)
- Pull-to-refresh
- Filter by status (unread/all) and priority

### Real-Time Delivery
- Subscribes to `GET /api/notifications/stream` via SSE
- SSE connection managed in Rust (not in the webview) for reliability
- On new notification event:
  - Updates the in-app list via Tauri event bridge (`tauri::Emitter`)
  - If priority is `high` or `critical`, fires an Android system notification via `tauri-plugin-notification`
- On SSE disconnect: automatic reconnect with exponential backoff (1s, 2s, 4s, max 30s)
- Uses `Last-Event-ID` header on reconnect to catch missed events

### Polling Fallback
- If SSE connection fails repeatedly (>5 consecutive failures), falls back to polling
- Polls `GET /api/notifications/?since=<last_seen>` every 30 seconds
- Resumes SSE attempt every 5 minutes

### Background Execution
- Uses Android foreground service to keep SSE connection alive when app is backgrounded
- Persistent notification: "priority-notify is running" (required by Android for foreground services)
- User can disable background mode in app settings (falls back to periodic polling via Android WorkManager)

### Settings
- Server URL
- Re-scan / re-enter API token
- Enable/disable background mode
- Enable/disable system notifications
- Notification priority threshold (only notify for high+critical, or all)

## Project Structure

```
./
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs            # Tauri app setup, command registration
│   │   ├── commands.rs       # Tauri commands (fetch notifications, mark read, etc.)
│   │   ├── sse.rs            # SSE client, reconnection logic, event parsing
│   │   ├── api.rs            # HTTP client wrapper for server API
│   │   └── models.rs         # Notification, Token, User structs (serde)
│   ├── Cargo.toml
│   ├── tauri.conf.json       # Tauri config (app name, permissions, plugins)
│   ├── capabilities/
│   │   └── default.json      # Tauri capability permissions
│   └── gen/
│       └── android/          # Generated Android project (Gradle, manifests)
├── src/                      # Webview frontend
│   ├── index.html            # Main app shell
│   ├── style.css             # Styling (priority colors, layout)
│   └── main.js               # UI logic, Tauri event listeners, invoke commands
├── .github/
│   └── workflows/
│       └── android.yml       # Build + sign APK
└── android.spec.md
```

## Android Permissions

| Permission | Reason |
|-----------|--------|
| `INTERNET` | API calls and SSE |
| `POST_NOTIFICATIONS` | System notifications (Android 13+ requires runtime permission) |
| `FOREGROUND_SERVICE` | Keep SSE alive in background |
| `FOREGROUND_SERVICE_DATA_SYNC` | Required service type for data sync |
| `CAMERA` | QR code scanning for token setup |

## API Integration

The app uses these server endpoints (see `server.spec.md` for full details):

| Endpoint | Usage |
|----------|-------|
| `GET /api/me` | Validate token on setup |
| `GET /api/notifications/` | Fetch notification list (with `?since=`, `?status=`, `?limit=`) |
| `GET /api/notifications/stream` | SSE subscription |
| `PATCH /api/notifications/{id}` | Mark read/unread |
| `DELETE /api/notifications/{id}` | Delete notification |

**Auth header on all requests:** `Authorization: Bearer <api-token>`

## GitHub Actions CI

### Workflow: `android.yml`

**Triggers:** Push to `main`, pull requests, manual dispatch.

**Steps:**
1. **Checkout** repository
2. **Setup Rust** toolchain + Android targets (`aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`)
3. **Setup Java** (JDK 17 for Gradle)
4. **Setup Android SDK** + NDK (via `android-actions/setup-android`)
5. **Install Tauri CLI**: `cargo install tauri-cli`
6. **Install frontend deps**: None needed (plain HTML/CSS/JS, no build step)
7. **Build APK**: `cargo tauri android build --apk`
8. **Sign APK**: Use `jarsigner` or `apksigner` with keystore from GitHub secrets
   - Secrets: `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`
9. **Upload artifact**: Upload signed APK as GitHub Actions artifact
10. **Release** (on tag push): Create GitHub Release with APK attached

### Secrets Required
| Secret | Description |
|--------|-------------|
| `ANDROID_KEYSTORE_BASE64` | Base64-encoded `.jks` keystore file |
| `ANDROID_KEYSTORE_PASSWORD` | Keystore password |
| `ANDROID_KEY_ALIAS` | Key alias within the keystore |
| `ANDROID_KEY_PASSWORD` | Key password |

### Build Targets
- `aarch64-linux-android` (ARM64, most modern devices)
- `armv7-linux-androideabi` (ARM32, older devices)
- `x86_64-linux-android` (emulators)

Output: Universal APK or per-ABI split APKs (TBD based on size).

## Open Questions

1. **APK size**: Tauri mobile APKs can be 10-20MB. Acceptable, or should we pursue split APKs per ABI?
2. **Foreground service**: Some OEMs (Xiaomi, Samsung) aggressively kill foreground services. May need to document device-specific battery optimization whitelisting.
3. **Token rotation**: Should the app support automatic token refresh, or is manual re-setup acceptable?
