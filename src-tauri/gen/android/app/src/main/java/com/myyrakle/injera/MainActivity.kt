package com.myyrakle.injera

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat

/**
 * The app browses and rewrites the user's own archives, which live in shared
 * storage. Without access every listing fails with EACCES, so ask on startup
 * rather than letting the file list come up empty.
 */
class MainActivity : TauriActivity() {
  /** Asked once per launch; re-opening Settings on every resume would trap the user. */
  private var asked = false

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    requestStorageAccess()
  }

  private fun requestStorageAccess() {
    if (asked) {
      return
    }
    asked = true

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
      if (!Environment.isExternalStorageManager()) {
        openAllFilesAccessSettings()
      }
      return
    }

    val read = Manifest.permission.READ_EXTERNAL_STORAGE
    if (ContextCompat.checkSelfPermission(this, read) != PackageManager.PERMISSION_GRANTED) {
      ActivityCompat.requestPermissions(this, arrayOf(read), STORAGE_REQUEST)
    }
  }

  /**
   * All files access is a Settings screen, not a dialog. The per-app screen is
   * not guaranteed to exist on every build, so fall back to the full list.
   */
  private fun openAllFilesAccessSettings() {
    val perApp = Intent(
      Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
      Uri.parse("package:$packageName"),
    )

    if (perApp.resolveActivity(packageManager) != null) {
      startActivity(perApp)
      return
    }

    val allApps = Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION)
    if (allApps.resolveActivity(packageManager) != null) {
      startActivity(allApps)
    }
  }

  private companion object {
    const val STORAGE_REQUEST = 1001
  }
}
