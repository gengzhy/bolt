package xin.cosmos.bolt

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import xin.cosmos.bolt.engine.BtEngine
import xin.cosmos.bolt.engine.NsdHelper
import xin.cosmos.bolt.engine.SharePayloadHelper
import xin.cosmos.bolt.ui.MainScreen
import xin.cosmos.bolt.ui.theme.BtTheme

/**
 * 唯一 Activity（需求 §2.6 权限申请 + 页面渲染）。
 *
 * 引擎/发现/任务都在进程级 [BtEngine] 中，本页面销毁不影响传输；
 * 权限到手后重试 NSD 发现（API 33+ NsdManager 需要 NEARBY_WIFI_DEVICES）。
 */
class MainActivity : ComponentActivity() {

    private val permissionLauncher =
        registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { grants ->
            if (grants.values.any { it }) {
                // 授权前启动的发现可能因权限被拒，这里重试
                NsdHelper.ensureDiscovery()
            }
            if (grants[Manifest.permission.POST_NOTIFICATIONS] == true) {
                PersistentNotification.show(this)
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        ensurePermissions()
        ensureStoragePermission()
        SharePayloadHelper.handleShareIntent(this, intent)
        setContent {
            BtTheme {
                MainScreen()
            }
        }
        // 回到前台时与核心任务表对账一次
        BtEngine.syncTasks()
        PersistentNotification.show(this)
    }

    override fun onResume() {
        super.onResume()
        PersistentNotification.show(this)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        SharePayloadHelper.handleShareIntent(this, intent)
        BtEngine.syncTasks()
        PersistentNotification.show(this)
    }

    private fun ensurePermissions() {
        val needed = mutableListOf<String>()
        if (Build.VERSION.SDK_INT >= 33) {
            if (!granted(Manifest.permission.NEARBY_WIFI_DEVICES)) {
                needed += Manifest.permission.NEARBY_WIFI_DEVICES
            }
            if (!granted(Manifest.permission.POST_NOTIFICATIONS)) {
                needed += Manifest.permission.POST_NOTIFICATIONS
            }
        } else if (Build.VERSION.SDK_INT >= 29) {
            if (!granted(Manifest.permission.ACCESS_FINE_LOCATION)) {
                needed += Manifest.permission.ACCESS_FINE_LOCATION
            }
            if (Build.VERSION.SDK_INT <= 29 &&
                !granted(Manifest.permission.WRITE_EXTERNAL_STORAGE)
            ) {
                needed += Manifest.permission.WRITE_EXTERNAL_STORAGE
            }
        }
        if (needed.isNotEmpty()) {
            permissionLauncher.launch(needed.toTypedArray())
        }
    }

    /**
     * 接收文件默认写入公共下载目录（/Download/Bolt）：
     * Android 11+ 需要「所有文件访问」特殊权限（跳系统设置页授权）。
     */
    private fun ensureStoragePermission() {
        if (Build.VERSION.SDK_INT < 30) return
        if (Environment.isExternalStorageManager()) return
        val target = try {
            Intent(
                Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                Uri.parse("package:$packageName"),
            )
        } catch (_: Exception) {
            null
        } ?: Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION)
        try {
            startActivity(target)
            Toast
                .makeText(
                    this,
                    getString(R.string.perm_storage_toast),
                    Toast.LENGTH_LONG,
                )
                .show()
        } catch (_: Exception) {
            Toast
                .makeText(this, getString(R.string.perm_storage_guide), Toast.LENGTH_LONG)
                .show()
        }
    }

    private fun granted(permission: String): Boolean =
        ContextCompat.checkSelfPermission(this, permission) == PackageManager.PERMISSION_GRANTED
}
