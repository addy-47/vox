package com.addy.Vox

import android.content.Context
import android.net.wifi.WifiManager
import android.util.Log

object MulticastLockManager {
    private const val TAG = "MulticastLockManager"
    private var lock: WifiManager.MulticastLock? = null

    @JvmStatic
    @Synchronized
    fun acquire(context: Context): Boolean {
        return try {
            if (lock == null) {
                val wifi = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
                lock = wifi?.createMulticastLock("vox_mdns_lock")?.apply {
                    setReferenceCounted(true)
                }
            }
            lock?.let {
                it.acquire()
                Log.i(TAG, "MulticastLock acquired (isHeld=${it.isHeld})")
                true
            } ?: false
        } catch (e: Exception) {
            Log.e(TAG, "Failed to acquire MulticastLock", e)
            false
        }
    }

    @JvmStatic
    @Synchronized
    fun release() {
        try {
            lock?.let {
                if (it.isHeld) {
                    it.release()
                    Log.i(TAG, "MulticastLock released (isHeld=${it.isHeld})")
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to release MulticastLock", e)
        }
    }
}

