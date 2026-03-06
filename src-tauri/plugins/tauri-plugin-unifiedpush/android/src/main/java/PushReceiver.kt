package page.osmosis.unifiedpush

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import org.json.JSONObject
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import org.unifiedpush.android.connector.MessagingReceiver
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL

class PushReceiver : MessagingReceiver() {

    companion object {
        const val TAG = "PushReceiver"
        const val PREFS_NAME = "unifiedpush_prefs"
        const val CHANNEL_ID = "push_notifications"
        const val CHANNEL_NAME = "Push Notifications"
        const val GROUP_KEY = "page.osmosis.prioritynotify.PUSH_GROUP"
        const val SUMMARY_ID = 9000

        private val PRIORITY_ORDER = mapOf(
            "all" to 0,
            "low" to 0,
            "medium" to 1,
            "high" to 2,
            "critical" to 3
        )
    }

    override fun onNewEndpoint(context: Context, endpoint: String, instance: String) {
        Log.d(TAG, "New endpoint received: $endpoint")

        val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        prefs.edit()
            .putString("endpoint", endpoint)
            .putString("status", "endpoint_received")
            .apply()

        val serverUrl = prefs.getString("server_url", "") ?: ""
        val token = prefs.getString("token", "") ?: ""

        if (serverUrl.isNotEmpty() && token.isNotEmpty()) {
            val pendingResult = goAsync()
            CoroutineScope(Dispatchers.IO).launch {
                try {
                    registerEndpointWithServer(serverUrl, token, endpoint)
                    Log.d(TAG, "Endpoint registered with server")
                    prefs.edit().putString("status", "active").apply()
                } catch (e: Exception) {
                    Log.e(TAG, "Failed to register endpoint with server", e)
                } finally {
                    pendingResult.finish()
                }
            }
        }
    }

    override fun onMessage(context: Context, message: ByteArray, instance: String) {
        Log.d(TAG, "Push message received")

        val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        val systemNotif = prefs.getBoolean("system_notif", true)
        val thresholdStr = prefs.getString("notif_threshold", "high") ?: "high"

        if (!systemNotif) return

        val thresholdLevel = PRIORITY_ORDER[thresholdStr] ?: 2

        try {
            val json = JSONObject(String(message, Charsets.UTF_8))

            val id = json.optString("id", "")
            val title = json.optString("title", "New notification")
            val messageText = json.optString("message", "")
            val priority = json.optString("priority", "medium")

            val priorityLevel = PRIORITY_ORDER[priority] ?: 0
            if (priorityLevel < thresholdLevel) return

            // Check notification permission on Android 13+
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                ContextCompat.checkSelfPermission(
                    context, Manifest.permission.POST_NOTIFICATIONS
                ) != PackageManager.PERMISSION_GRANTED
            ) {
                return
            }

            createNotificationChannel(context)
            showNotification(context, id, title, messageText, priority)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse push message", e)
        }
    }

    override fun onUnregistered(context: Context, instance: String) {
        Log.d(TAG, "Unregistered from push")

        val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        prefs.edit()
            .putString("endpoint", "")
            .putString("status", "unregistered")
            .apply()
    }

    override fun onRegistrationFailed(context: Context, instance: String) {
        Log.e(TAG, "Registration failed")

        val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        prefs.edit()
            .putString("status", "failed")
            .apply()
    }

    private fun registerEndpointWithServer(serverUrl: String, token: String, endpoint: String) {
        val baseUrl = serverUrl.trimEnd('/')
        val url = URL("$baseUrl/api/push/register")
        val conn = url.openConnection() as HttpURLConnection
        conn.requestMethod = "POST"
        conn.setRequestProperty("Authorization", "Bearer $token")
        conn.setRequestProperty("Content-Type", "application/json")
        conn.setRequestProperty("Accept", "application/json")
        conn.connectTimeout = 30_000
        conn.readTimeout = 30_000
        conn.doOutput = true

        try {
            val body = JSONObject().apply {
                put("endpoint", endpoint)
            }
            val writer = OutputStreamWriter(conn.outputStream)
            writer.write(body.toString())
            writer.flush()
            writer.close()

            val responseCode = conn.responseCode
            if (responseCode in 200..299) {
                Log.d(TAG, "Endpoint registered successfully")
            } else {
                val reader = BufferedReader(InputStreamReader(conn.errorStream ?: conn.inputStream))
                val responseBody = reader.readText()
                reader.close()
                Log.e(TAG, "Server returned $responseCode: $responseBody")
            }
        } finally {
            conn.disconnect()
        }
    }

    private fun createNotificationChannel(context: Context) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                CHANNEL_NAME,
                NotificationManager.IMPORTANCE_HIGH
            ).apply {
                description = "Notifications received via push"
            }

            val manager = context.getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(channel)
        }
    }

    private fun showNotification(
        context: Context,
        id: String,
        title: String,
        message: String,
        priority: String
    ) {
        val notifManager = NotificationManagerCompat.from(context)
        val iconRes = context.applicationInfo.icon

        val androidPriority = when (priority) {
            "critical" -> NotificationCompat.PRIORITY_MAX
            "high" -> NotificationCompat.PRIORITY_HIGH
            "medium" -> NotificationCompat.PRIORITY_DEFAULT
            else -> NotificationCompat.PRIORITY_LOW
        }

        val builder = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(iconRes)
            .setContentTitle(title)
            .setContentText(message.ifEmpty { null })
            .setPriority(androidPriority)
            .setAutoCancel(true)
            .setGroup(GROUP_KEY)

        try {
            notifManager.notify(id.hashCode(), builder.build())
        } catch (e: SecurityException) {
            return
        }
    }
}
