package page.osmosis.backgroundcheck

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import org.json.JSONArray
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import java.text.SimpleDateFormat
import java.util.*

class NotificationCheckWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    companion object {
        const val CHANNEL_ID = "background_notifications"
        const val CHANNEL_NAME = "Background Notifications"
        const val GROUP_KEY = "page.osmosis.prioritynotify.BACKGROUND_GROUP"
        const val SUMMARY_ID = 0

        private val PRIORITY_ORDER = mapOf(
            "low" to 0,
            "medium" to 1,
            "high" to 2,
            "critical" to 3
        )
    }

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        val prefs = applicationContext.getSharedPreferences(
            BackgroundCheckPlugin.PREFS_NAME, Context.MODE_PRIVATE
        )

        val serverUrl = prefs.getString("server_url", "") ?: ""
        val token = prefs.getString("token", "") ?: ""
        val systemNotif = prefs.getBoolean("system_notif", true)
        val thresholdStr = prefs.getString("notif_threshold", "high") ?: "high"
        val lastCheckTime = prefs.getString("last_check_time", null)

        // Exit early if notifications disabled or credentials missing
        if (!systemNotif || serverUrl.isEmpty() || token.isEmpty()) {
            return@withContext Result.success()
        }

        val thresholdLevel = PRIORITY_ORDER[thresholdStr] ?: 2

        try {
            val notifications = fetchUnreadNotifications(serverUrl, token, lastCheckTime)

            // Filter by priority threshold
            val filtered = notifications.filter { notif ->
                val priority = notif.optString("priority", "low")
                (PRIORITY_ORDER[priority] ?: 0) >= thresholdLevel
            }

            if (filtered.isNotEmpty()) {
                // Check notification permission on Android 13+
                if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
                    ContextCompat.checkSelfPermission(
                        applicationContext, Manifest.permission.POST_NOTIFICATIONS
                    ) == PackageManager.PERMISSION_GRANTED
                ) {
                    createNotificationChannel()
                    showNotifications(filtered)
                }
            }

            // Update last check time
            val iso8601 = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", Locale.US).apply {
                timeZone = TimeZone.getTimeZone("UTC")
            }
            prefs.edit()
                .putString("last_check_time", iso8601.format(Date()))
                .apply()

            Result.success()
        } catch (e: java.net.UnknownHostException) {
            Result.retry()
        } catch (e: java.net.SocketTimeoutException) {
            Result.retry()
        } catch (e: java.io.IOException) {
            Result.retry()
        } catch (e: Exception) {
            Result.failure()
        }
    }

    private fun fetchUnreadNotifications(
        serverUrl: String,
        token: String,
        since: String?
    ): List<JSONObject> {
        val baseUrl = serverUrl.trimEnd('/')
        val params = StringBuilder("status=unread&limit=50")
        if (!since.isNullOrEmpty()) {
            params.append("&since=")
            params.append(URLEncoder.encode(since, "UTF-8"))
        }

        val url = URL("$baseUrl/api/notifications/?$params")
        val conn = url.openConnection() as HttpURLConnection
        conn.requestMethod = "GET"
        conn.setRequestProperty("Authorization", "Bearer $token")
        conn.setRequestProperty("Accept", "application/json")
        conn.connectTimeout = 30_000
        conn.readTimeout = 30_000

        try {
            val responseCode = conn.responseCode
            if (responseCode != 200) {
                return emptyList()
            }

            val reader = BufferedReader(InputStreamReader(conn.inputStream))
            val body = reader.readText()
            reader.close()

            val json = JSONObject(body)
            val items = json.getJSONArray("items")
            val result = mutableListOf<JSONObject>()
            for (i in 0 until items.length()) {
                result.add(items.getJSONObject(i))
            }
            return result
        } finally {
            conn.disconnect()
        }
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                CHANNEL_NAME,
                NotificationManager.IMPORTANCE_HIGH
            ).apply {
                description = "Notifications from background check"
            }

            val manager = applicationContext.getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(channel)
        }
    }

    private fun showNotifications(notifications: List<JSONObject>) {
        val notifManager = NotificationManagerCompat.from(applicationContext)
        val iconRes = applicationContext.applicationInfo.icon

        for (notif in notifications) {
            val id = notif.optString("id", "")
            val title = notif.optString("title", "New notification")
            val message = notif.optString("message", "")
            val priority = notif.optString("priority", "medium")

            val androidPriority = when (priority) {
                "critical" -> NotificationCompat.PRIORITY_MAX
                "high" -> NotificationCompat.PRIORITY_HIGH
                "medium" -> NotificationCompat.PRIORITY_DEFAULT
                else -> NotificationCompat.PRIORITY_LOW
            }

            val builder = NotificationCompat.Builder(applicationContext, CHANNEL_ID)
                .setSmallIcon(iconRes)
                .setContentTitle(title)
                .setContentText(message.ifEmpty { null })
                .setPriority(androidPriority)
                .setAutoCancel(true)
                .setGroup(GROUP_KEY)

            try {
                notifManager.notify(id.hashCode(), builder.build())
            } catch (e: SecurityException) {
                // Permission revoked at runtime
                return
            }
        }

        // Group summary if multiple notifications
        if (notifications.size > 1) {
            val summary = NotificationCompat.Builder(applicationContext, CHANNEL_ID)
                .setSmallIcon(iconRes)
                .setContentTitle("Priority Notify")
                .setContentText("${notifications.size} new notifications")
                .setGroup(GROUP_KEY)
                .setGroupSummary(true)
                .setAutoCancel(true)
                .build()

            try {
                notifManager.notify(SUMMARY_ID, summary)
            } catch (e: SecurityException) {
                // Permission revoked at runtime
            }
        }
    }
}
