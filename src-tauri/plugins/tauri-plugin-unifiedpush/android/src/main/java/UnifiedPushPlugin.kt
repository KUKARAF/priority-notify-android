package page.osmosis.unifiedpush

import android.app.Activity
import android.content.Context
import android.content.SharedPreferences
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.unifiedpush.android.connector.UnifiedPush

@InvokeArg
class RegisterArgs {
    var distributor: String? = null
    var instance: String = "default"
}

@InvokeArg
class SaveCredentialsArgs {
    var serverUrl: String = ""
    var token: String = ""
    var notifThreshold: String = "high"
    var systemNotif: Boolean = true
}

@TauriPlugin
class UnifiedPushPlugin(private val activity: Activity) : Plugin(activity) {

    private fun getPrefs(): SharedPreferences {
        return activity.getSharedPreferences(PushReceiver.PREFS_NAME, Context.MODE_PRIVATE)
    }

    @Command
    fun getDistributors(invoke: Invoke) {
        val distributors = UnifiedPush.getDistributors(activity.applicationContext)

        val result = JSObject()
        val arr = org.json.JSONArray()
        for (d in distributors) {
            arr.put(d)
        }
        result.put("distributors", arr)
        result.put("count", distributors.size)
        invoke.resolve(result)
    }

    @Command
    fun register(invoke: Invoke) {
        val args = invoke.parseArgs(RegisterArgs::class.java)

        val distributors = UnifiedPush.getDistributors(activity.applicationContext)
        if (distributors.isEmpty()) {
            invoke.reject("No UnifiedPush distributor installed")
            return
        }

        // Pick distributor: use specified, or first available
        val distributor = if (!args.distributor.isNullOrEmpty() && distributors.contains(args.distributor)) {
            args.distributor!!
        } else {
            distributors[0]
        }

        getPrefs().edit()
            .putString("status", "registering")
            .putString("distributor", distributor)
            .apply()

        UnifiedPush.saveDistributor(activity.applicationContext, distributor)
        UnifiedPush.register(activity.applicationContext, args.instance)

        invoke.resolve()
    }

    @Command
    fun unregister(invoke: Invoke) {
        try {
            UnifiedPush.unregister(activity.applicationContext)
        } catch (e: Exception) {
            // May fail if no distributor set; that's fine
        }

        getPrefs().edit()
            .remove("endpoint")
            .remove("status")
            .remove("distributor")
            .apply()

        invoke.resolve()
    }

    @Command
    fun getPushStatus(invoke: Invoke) {
        val prefs = getPrefs()
        val distributors = UnifiedPush.getDistributors(activity.applicationContext)

        val result = JSObject()
        result.put("status", prefs.getString("status", "inactive") ?: "inactive")
        result.put("endpoint", prefs.getString("endpoint", "") ?: "")
        result.put("hasDistributor", distributors.isNotEmpty())
        result.put("distributorCount", distributors.size)
        result.put("distributor", prefs.getString("distributor", "") ?: "")
        invoke.resolve(result)
    }

    @Command
    fun saveCredentials(invoke: Invoke) {
        val args = invoke.parseArgs(SaveCredentialsArgs::class.java)

        getPrefs().edit()
            .putString("server_url", args.serverUrl)
            .putString("token", args.token)
            .putString("notif_threshold", args.notifThreshold)
            .putBoolean("system_notif", args.systemNotif)
            .apply()

        invoke.resolve()
    }
}
