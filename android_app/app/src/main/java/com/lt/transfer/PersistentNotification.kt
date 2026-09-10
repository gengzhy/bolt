package com.lt.transfer

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat

/**
 * 常驻通知管理（类似 QQ、微信等主流 App 的常驻通知栏状态）。
 *
 * 遵循最小化改动原则：
 * - 独立工具类，不干扰现有 TransferService 的传输前台逻辑；
 * - 使用低优先级通知渠道（IMPORTANCE_LOW），不发声不振动；
 * - 点击后直接跳转唤起 MainActivity；
 * - 设置 ongoing = true，保持常驻。
 */
object PersistentNotification {
    private const val CHANNEL_ID = "lt_persistent"
    private const val NOTIFICATION_ID = 10001

    fun show(context: Context) {
        // Android 13+ 检查 POST_NOTIFICATIONS 权限
        if (Build.VERSION.SDK_INT >= 33) {
            val granted = ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.POST_NOTIFICATIONS,
            ) == PackageManager.PERMISSION_GRANTED
            if (!granted) return
        }

        try {
            val nm = context.getSystemService(NotificationManager::class.java) ?: return

            val channel = NotificationChannel(
                CHANNEL_ID,
                "后台常驻运行",
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = "保持 LocalTransfer 后台运行并随时快捷访问"
                setShowBadge(false)
            }
            nm.createNotificationChannel(channel)

            val launchIntent = Intent(context, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
            }
            val pendingIntent = PendingIntent.getActivity(
                context,
                0,
                launchIntent,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )

            val notification = NotificationCompat.Builder(context, CHANNEL_ID)
                .setSmallIcon(R.mipmap.ic_launcher)
                .setContentTitle(context.getString(R.string.app_name))
                .setContentText("LocalTransfer 运行中，点击快速进入")
                .setOngoing(true)
                .setShowWhen(false)
                .setContentIntent(pendingIntent)
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .build()

            nm.notify(NOTIFICATION_ID, notification)
        } catch (_: Exception) {
            // 避免因系统特殊权限限制导致 crash
        }
    }

    fun dismiss(context: Context) {
        try {
            val nm = context.getSystemService(NotificationManager::class.java)
            nm?.cancel(NOTIFICATION_ID)
        } catch (_: Exception) {
        }
    }
}
