package com.addy.Vox

import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import app.tauri.plugin.PluginManager

class MainActivity : TauriActivity() {
  companion object {
    init {
      System.loadLibrary("vox_lib")
    }
  }

  private external fun initAndroidContext()

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    PluginManager.onActivityCreate(this)
    initAndroidContext()
  }
}
