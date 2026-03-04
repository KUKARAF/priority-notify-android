package page.osmosis.backgroundcheck

import android.app.Activity
import android.content.Context
import android.content.SharedPreferences
import androidx.work.*
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.util.concurrent.TimeUnit

@InvokeArg
class ScheduleArgs {
    var intervalMinutes: Int = 15
    var serverUrl: String = ""
    var token: String = ""
    var notifThreshold: String = "high"
    var systemNotif: Boolean = true
}

@InvokeArg
class UpdateSettingsArgs {
    var notifThreshold: String? = null
    var systemNotif: Boolean? = null
}

@TauriPlugin
class BackgroundCheckPlugin(private val activity: Activity) : Plugin(activity) {

    companion object {
        const val WORK_NAME = "background_notification_check"
        const val PREFS_NAME = "background_check_prefs"
    }

    private fun getPrefs(): SharedPreferences {
        return activity.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    }

    @Command
    fun schedule(invoke: Invoke) {
        val args = invoke.parseArgs(ScheduleArgs::class.java)

        // Save config to SharedPreferences so the Worker can read it
        getPrefs().edit()
            .putString("server_url", args.serverUrl)
            .putString("token", args.token)
            .putString("notif_threshold", args.notifThreshold)
            .putBoolean("system_notif", args.systemNotif)
            .apply()

        val interval = maxOf(args.intervalMinutes.toLong(), 15L)

        val constraints = Constraints.Builder()
            .setRequiredNetworkType(NetworkType.CONNECTED)
            .build()

        val workRequest = PeriodicWorkRequestBuilder<NotificationCheckWorker>(
            interval, TimeUnit.MINUTES
        )
            .setConstraints(constraints)
            .setBackoffCriteria(
                BackoffPolicy.EXPONENTIAL,
                WorkRequest.MIN_BACKOFF_MILLIS,
                TimeUnit.MILLISECONDS
            )
            .build()

        WorkManager.getInstance(activity.applicationContext)
            .enqueueUniquePeriodicWork(
                WORK_NAME,
                ExistingPeriodicWorkPolicy.UPDATE,
                workRequest
            )

        invoke.resolve()
    }

    @Command
    fun cancel(invoke: Invoke) {
        WorkManager.getInstance(activity.applicationContext)
            .cancelUniqueWork(WORK_NAME)

        getPrefs().edit().clear().apply()

        invoke.resolve()
    }

    @Command
    fun updateSettings(invoke: Invoke) {
        val args = invoke.parseArgs(UpdateSettingsArgs::class.java)
        val editor = getPrefs().edit()

        args.notifThreshold?.let { editor.putString("notif_threshold", it) }
        args.systemNotif?.let { editor.putBoolean("system_notif", it) }

        editor.apply()
        invoke.resolve()
    }
}
