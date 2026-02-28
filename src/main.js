// ── Tauri API ──────────────────────────────────────────
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// ── DOM References ─────────────────────────────────────
const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

const views = {
  setup: $("#setup-view"),
  main: $("#main-view"),
  settings: $("#settings-view"),
};

const els = {
  serverUrl: $("#server-url"),
  apiToken: $("#api-token"),
  scanQrBtn: $("#scan-qr-btn"),
  connectBtn: $("#connect-btn"),
  setupError: $("#setup-error"),
  setupLoading: $("#setup-loading"),

  sseIndicator: $("#sse-indicator"),
  settingsBtn: $("#settings-btn"),
  statusFilter: $("#status-filter"),
  priorityFilter: $("#priority-filter"),
  notifList: $("#notification-list"),
  emptyState: $("#empty-state"),
  listLoading: $("#list-loading"),

  detailModal: $("#detail-modal"),
  detailPriority: $("#detail-priority"),
  detailClose: $("#detail-close"),
  detailTitle: $("#detail-title"),
  detailSource: $("#detail-source"),
  detailTime: $("#detail-time"),
  detailBody: $("#detail-body"),
  detailToggleRead: $("#detail-toggle-read"),
  detailDelete: $("#detail-delete"),

  settingsBack: $("#settings-back"),
  settingsServerUrl: $("#settings-server-url"),
  settingsUser: $("#settings-user"),
  settingsReconnect: $("#settings-reconnect"),
  settingsSystemNotif: $("#settings-system-notif"),
  settingsNotifThreshold: $("#settings-notif-threshold"),
  settingsReminder: $("#settings-reminder"),
  settingsBackground: $("#settings-background"),
  settingsLogout: $("#settings-logout"),
};

// ── App State ──────────────────────────────────────────
let notifications = [];
let currentUser = null;
let selectedNotif = null;

// ── View Management ────────────────────────────────────
function showView(name) {
  Object.values(views).forEach((v) => v.classList.add("hidden"));
  views[name].classList.remove("hidden");
}

// ── Utilities ──────────────────────────────────────────
function timeAgo(dateStr) {
  const now = Date.now();
  const then = new Date(dateStr).getTime();
  const secs = Math.floor((now - then) / 1000);

  if (secs < 60) return "just now";
  if (secs < 3600) return Math.floor(secs / 60) + "m ago";
  if (secs < 86400) return Math.floor(secs / 3600) + "h ago";
  return Math.floor(secs / 86400) + "d ago";
}

function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}

// ── Notification Rendering ─────────────────────────────
function renderNotifications() {
  const list = els.notifList;

  if (notifications.length === 0) {
    list.innerHTML = "";
    list.appendChild(els.emptyState);
    els.emptyState.classList.remove("hidden");
    return;
  }

  els.emptyState.classList.add("hidden");
  list.innerHTML = "";

  for (const notif of notifications) {
    const card = document.createElement("div");
    card.className = `notif-card${notif.status === "read" ? " read" : ""}`;
    card.dataset.id = notif.id;

    card.innerHTML = `
      <div class="notif-priority-bar ${notif.priority}"></div>
      <div class="notif-content">
        <div class="notif-title">${escapeHtml(notif.title)}</div>
        ${notif.message ? `<div class="notif-message">${escapeHtml(notif.message)}</div>` : ""}
        <div class="notif-meta">
          ${notif.source ? `<span class="notif-source">${escapeHtml(notif.source)}</span>` : ""}
          <span>${timeAgo(notif.created_at)}</span>
        </div>
      </div>
      <div class="notif-actions">
        <button class="notif-mark-btn" data-id="${notif.id}" title="${notif.status === "unread" ? "Mark read" : "Mark unread"}">
          ${notif.status === "unread" ? "&#9679;" : "&#9675;"}
        </button>
      </div>
    `;

    // Tap card -> open detail
    card.addEventListener("click", (e) => {
      if (e.target.closest(".notif-mark-btn")) return;
      openDetail(notif);
    });

    // Mark read/unread button
    card.querySelector(".notif-mark-btn").addEventListener("click", (e) => {
      e.stopPropagation();
      toggleRead(notif);
    });

    list.appendChild(card);
  }
}

// ── API Calls ──────────────────────────────────────────
async function loadNotifications() {
  els.listLoading.classList.remove("hidden");
  try {
    const status = els.statusFilter.value || null;
    const priority = els.priorityFilter.value || null;
    const result = await invoke("fetch_notifications", {
      status,
      priority,
      since: null,
      limit: 50,
      offset: null,
    });
    notifications = result.items;
    renderNotifications();
  } catch (e) {
    console.error("Failed to load notifications:", e);
  } finally {
    els.listLoading.classList.add("hidden");
  }
}

async function toggleRead(notif) {
  const newStatus = notif.status === "unread" ? "read" : "unread";
  try {
    const updated = await invoke("mark_notification", {
      id: notif.id,
      status: newStatus,
    });
    const idx = notifications.findIndex((n) => n.id === updated.id);
    if (idx >= 0) notifications[idx] = updated;
    renderNotifications();
  } catch (e) {
    console.error("Failed to update notification:", e);
  }
}

async function deleteNotification(id) {
  try {
    await invoke("delete_notification", { id });
    notifications = notifications.filter((n) => n.id !== id);
    renderNotifications();
    closeDetail();
  } catch (e) {
    console.error("Failed to delete notification:", e);
  }
}

// ── Detail Modal ───────────────────────────────────────
function openDetail(notif) {
  selectedNotif = notif;
  els.detailPriority.className = `priority-badge ${notif.priority}`;
  els.detailPriority.textContent = notif.priority;
  els.detailTitle.textContent = notif.title;
  els.detailSource.textContent = notif.source || "";
  els.detailTime.textContent = timeAgo(notif.created_at);
  els.detailBody.textContent = notif.message || "No message body.";
  els.detailToggleRead.textContent =
    notif.status === "unread" ? "Mark as read" : "Mark as unread";
  els.detailModal.classList.remove("hidden");

  // Auto-mark as read when opened
  if (notif.status === "unread") {
    toggleRead(notif);
  }
}

function closeDetail() {
  selectedNotif = null;
  els.detailModal.classList.add("hidden");
}

// ── Setup / Connection ─────────────────────────────────
async function connect() {
  const serverUrl = els.serverUrl.value.trim();
  const token = els.apiToken.value.trim();

  if (!serverUrl || !token) {
    showError("Please enter both server URL and token.");
    return;
  }

  els.setupError.classList.add("hidden");
  els.setupLoading.classList.remove("hidden");
  els.connectBtn.disabled = true;

  try {
    currentUser = await invoke("configure", {
      serverUrl: serverUrl,
      token: token,
    });
    await enterMainView();
  } catch (e) {
    showError(typeof e === "string" ? e : "Connection failed.");
  } finally {
    els.setupLoading.classList.add("hidden");
    els.connectBtn.disabled = false;
  }
}

function showError(msg) {
  els.setupError.textContent = msg;
  els.setupError.classList.remove("hidden");
}

async function enterMainView() {
  showView("main");
  els.settingsServerUrl.textContent = els.serverUrl.value.trim();
  els.settingsUser.textContent = currentUser
    ? `${currentUser.name} (${currentUser.email})`
    : "—";

  await loadNotifications();

  // Start SSE
  try {
    await invoke("start_sse");
  } catch (e) {
    console.error("SSE start failed:", e);
  }

  // Start reminder loop
  try {
    await invoke("start_reminder");
  } catch (e) {
    console.error("Reminder start failed:", e);
  }
}

// ── QR Scanner ─────────────────────────────────────────
async function scanQr() {
  const btn = els.scanQrBtn;
  const originalText = btn.innerHTML;

  try {
    btn.disabled = true;
    btn.innerHTML = '<span class="btn-icon">&#9634;</span> Checking camera...';

    // Check current camera permission state
    const permStatus = await invoke("plugin:barcode-scanner|check_permissions");
    let permission = permStatus.camera;

    // Request permission if not yet granted
    if (permission !== "granted") {
      if (permission === "denied") {
        showError(
          "Camera permission was denied. Please enable it in app settings."
        );
        try {
          await invoke("plugin:barcode-scanner|open_app_settings");
        } catch {
          // open_app_settings may not be available on all platforms
        }
        return;
      }

      const reqResult = await invoke(
        "plugin:barcode-scanner|request_permissions"
      );
      permission = reqResult.camera;

      if (permission !== "granted") {
        showError("Camera permission is required to scan QR codes.");
        return;
      }
    }

    // Perform the scan
    btn.innerHTML = '<span class="btn-icon">&#9634;</span> Scanning...';
    const result = await invoke("plugin:barcode-scanner|scan", {
      formats: ["QR_CODE"],
      windowed: false,
    });

    if (result && result.content) {
      try {
        const data = JSON.parse(result.content);
        if (data.server_url) els.serverUrl.value = data.server_url;
        if (data.token) els.apiToken.value = data.token;
      } catch {
        // Not JSON — treat as plain token value
        els.apiToken.value = result.content;
      }
    }
  } catch (e) {
    console.error("QR scan failed:", e);
    const msg =
      typeof e === "string"
        ? e
        : "QR scan failed. Please paste your token manually.";
    showError(msg);
  } finally {
    btn.disabled = false;
    btn.innerHTML = originalText;
  }
}

// ── Pull to Refresh ────────────────────────────────────
function setupPullToRefresh() {
  const list = els.notifList;
  let startY = 0;
  let pulling = false;

  list.addEventListener("touchstart", (e) => {
    if (list.scrollTop === 0) {
      startY = e.touches[0].clientY;
      pulling = true;
    }
  });

  list.addEventListener("touchmove", (e) => {
    if (!pulling) return;
    const diff = e.touches[0].clientY - startY;
    if (diff > 80) {
      $("#pull-indicator").classList.remove("hidden");
    }
  });

  list.addEventListener("touchend", () => {
    if (!pulling) return;
    pulling = false;
    const indicator = $("#pull-indicator");
    if (!indicator.classList.contains("hidden")) {
      indicator.classList.add("hidden");
      loadNotifications();
    }
  });
}

// ── SSE Status ─────────────────────────────────────────
function updateSseStatus(status) {
  const dot = els.sseIndicator;
  dot.className = "sse-dot";

  if (status === "connected") {
    dot.classList.add("connected");
    dot.title = "Connected (live)";
  } else if (status === "polling") {
    dot.classList.add("polling");
    dot.title = "Polling mode";
  } else if (typeof status === "object" && status.status === "reconnecting") {
    dot.classList.add("reconnecting");
    dot.title = `Reconnecting in ${status.delay_secs}s`;
  } else {
    dot.classList.add("disconnected");
    dot.title = "Disconnected";
  }
}

// ── Event Listeners ────────────────────────────────────
function setupListeners() {
  els.connectBtn.addEventListener("click", connect);
  els.scanQrBtn.addEventListener("click", scanQr);
  els.settingsBtn.addEventListener("click", () => showView("settings"));
  els.settingsBack.addEventListener("click", () => showView("main"));
  els.detailClose.addEventListener("click", closeDetail);

  els.detailToggleRead.addEventListener("click", () => {
    if (selectedNotif) toggleRead(selectedNotif);
    closeDetail();
  });

  els.detailDelete.addEventListener("click", () => {
    if (selectedNotif) deleteNotification(selectedNotif.id);
  });

  // Close modal on backdrop click
  els.detailModal.addEventListener("click", (e) => {
    if (e.target === els.detailModal) closeDetail();
  });

  // Filters
  els.statusFilter.addEventListener("change", loadNotifications);
  els.priorityFilter.addEventListener("change", loadNotifications);

  // Settings: reconnect
  els.settingsReconnect.addEventListener("click", async () => {
    await invoke("stop_sse");
    await invoke("stop_reminder");
    showView("setup");
  });

  // Settings: logout
  els.settingsLogout.addEventListener("click", async () => {
    await invoke("stop_sse");
    await invoke("stop_reminder");
    await invoke("save_setting", { key: "server_url", value: "" });
    await invoke("save_setting", { key: "token", value: "" });
    els.serverUrl.value = "";
    els.apiToken.value = "";
    notifications = [];
    currentUser = null;
    showView("setup");
  });

  // Settings: persist toggles
  els.settingsSystemNotif.addEventListener("change", (e) => {
    invoke("save_setting", {
      key: "system_notif",
      value: e.target.checked ? "true" : "false",
    });
  });

  els.settingsNotifThreshold.addEventListener("change", (e) => {
    invoke("save_setting", { key: "notif_threshold", value: e.target.value });
  });

  els.settingsReminder.addEventListener("change", (e) => {
    const minutes = parseInt(e.target.value, 10);
    invoke("update_reminder_interval", { minutes });
  });

  els.settingsBackground.addEventListener("change", (e) => {
    invoke("save_setting", {
      key: "background",
      value: e.target.checked ? "true" : "false",
    });
  });

  // Enter key on token field
  els.apiToken.addEventListener("keydown", (e) => {
    if (e.key === "Enter") connect();
  });

  setupPullToRefresh();
}

// ── Tauri Event Bridge ─────────────────────────────────
async function setupTauriEvents() {
  await listen("new-notification", (event) => {
    const notif = event.payload;
    // Add to list if not already present
    const existing = notifications.findIndex((n) => n.id === notif.id);
    if (existing >= 0) {
      notifications[existing] = notif;
    } else {
      notifications.unshift(notif);
    }
    renderNotifications();
  });

  await listen("status-change", (event) => {
    const { id, status } = event.payload;
    const notif = notifications.find((n) => n.id === id);
    if (notif) {
      notif.status = status;
      renderNotifications();
    }
  });

  await listen("sse-status", (event) => {
    updateSseStatus(event.payload);
  });
}

// ── Init ───────────────────────────────────────────────
async function init() {
  setupListeners();
  await setupTauriEvents();

  // Try to restore saved settings
  try {
    const settings = await invoke("load_settings");
    if (settings) {
      const [url, token] = settings;
      els.serverUrl.value = url;
      els.apiToken.value = token;

      // Auto-connect with saved credentials
      els.setupLoading.classList.remove("hidden");
      try {
        currentUser = await invoke("configure", {
          serverUrl: url,
          token: token,
        });
        await enterMainView();
        return;
      } catch {
        // Saved credentials invalid, show setup
        els.setupLoading.classList.add("hidden");
      }
    }
  } catch {
    // No saved settings
  }

  // Load persisted preferences
  try {
    const sysNotif = await invoke("load_setting", { key: "system_notif" });
    if (sysNotif !== null) els.settingsSystemNotif.checked = sysNotif !== "false";

    const threshold = await invoke("load_setting", { key: "notif_threshold" });
    if (threshold) els.settingsNotifThreshold.value = threshold;

    const bg = await invoke("load_setting", { key: "background" });
    if (bg !== null) els.settingsBackground.checked = bg !== "false";

    const reminderInterval = await invoke("load_setting", {
      key: "reminder_interval",
    });
    if (reminderInterval !== null) els.settingsReminder.value = reminderInterval;
  } catch {
    // Defaults are fine
  }

  showView("setup");
}

document.addEventListener("DOMContentLoaded", init);
