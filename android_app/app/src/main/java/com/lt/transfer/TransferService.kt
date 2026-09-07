package com.lt.transfer

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import com.lt.transfer.engine.LtEngine
import com.lt.transfer.ffi.Native
import com.lt.transfer.model.TaskStates
import com.lt.transfer.model.TaskUi
import com.lt.transfer.ui.Format
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * 传输前台服务（需求 §2.6.2 后台常驻 + §2.6.4 通知栏进度与一键取消）。
 *
 * - 由 [LtEngine] 在出现活跃任务时 startForegroundService 拉起；
 * - 观察核心任务表：活跃任务逐个出带进度条的通知（通知栏「取消」
 *   透传 lt_cancel_task），任务结束换成完成通知；
 * - 无活跃任务后自动退出（stopForeground + stopSelf）。
 */
class TransferService : Service() {

    companion object {
        const val ACTION_CANCEL = "com.lt.transfer.action.CANCEL_TASK"
        const val EXTRA_TASK_ID = "task_id"
        private const val CHANNEL_ID = "lt_transfer"
        private const val CHANNEL_DONE_ID = "lt_transfer_done"
        /** 进度通知最小刷新间隔（避免高频事件刷屏）。 */
        private const val MIN_UPDATE_INTERVAL_MS = 800L
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    /** 正在展示通知的任务；终态时换成完成通知后移出。 */
    private val tracked = HashMap<Long, TaskUi>()
    private val lastNotifiedAt = HashMap<Long, Long>()
    private var foreground = false

    override fun onCreate() {
        super.onCreate()
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL_ID, "文件传输", NotificationManager.IMPORTANCE_LOW),
        )
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL_DONE_ID, "传输完成", NotificationManager.IMPORTANCE_DEFAULT),
        )

        scope.launch {
            LtEngine.uiState.collect { state ->
                onTasksChanged(state.tasks.values.toList())
            }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_CANCEL) {
            val taskId = intent.getLongExtra(EXTRA_TASK_ID, 0L)
            if (taskId != 0L) Native.ltCancelTask(taskId)
            return START_NOT_STICKY
        }
        // 首次进入前台：立即用当前活跃任务顶格（5s 限制）
        val active = activeTasks()
        if (active.isEmpty()) {
            stopSelf()
        } else {
            promoteForeground(active.first())
        }
        return START_NOT_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    // ---------------- 通知 ----------------

    private fun activeTasks(): List<TaskUi> =
        LtEngine.uiState.value.tasks.values.filter { TaskStates.isActive(it.state) }

    private fun onTasksChanged(tasks: List<TaskUi>) {
        val byId = tasks.associateBy { it.taskId }

        // 任务被核心清除（lt_clear_records）：直接摘除通知跟踪
        tracked.keys.filter { it !in byId }.forEach {
            tracked.remove(it)
            lastNotifiedAt.remove(it)
        }

        for (task in byId.values) {
            if (TaskStates.isActive(task.state)) {
                if (!tracked.containsKey(task.taskId) && !foreground) {
                    promoteForeground(task)
                }
                tracked[task.taskId] = task
                notifyActive(task)
            } else if (tracked.remove(task.taskId) != null) {
                notifyFinished(task)
            }
        }

        if (tracked.isEmpty() && foreground) {
            // 完成通知已就位，分离前台身份后退出
            ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_DETACH)
            foreground = false
            stopSelf()
        }
    }

    private fun promoteForeground(task: TaskUi) {
        ServiceCompat.startForeground(
            this,
            notifId(task.taskId),
            buildActiveNotification(task),
            ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
        )
        foreground = true
    }

    private fun notifyActive(task: TaskUi) {
        val now = System.currentTimeMillis()
        val last = lastNotifiedAt[task.taskId] ?: 0L
        if (now - last < MIN_UPDATE_INTERVAL_MS) return
        lastNotifiedAt[task.taskId] = now
        getSystemService(NotificationManager::class.java)
            .notify(notifId(task.taskId), buildActiveNotification(task))
    }

    private fun buildActiveNotification(task: TaskUi): Notification {
        val direction = if (task.incoming) "↓ 接收自" else "↑ 发送给"
        val peer = task.peerName.ifEmpty { task.peerUuid }
        val cancelIntent = PendingIntent.getService(
            this,
            notifId(task.taskId),
            Intent(this, TransferService::class.java)
                .setAction(ACTION_CANCEL)
                .putExtra(EXTRA_TASK_ID, task.taskId),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        val builder = NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(
                if (task.incoming) android.R.drawable.stat_sys_download
                else android.R.drawable.stat_sys_upload,
            )
            .setContentTitle("$direction $peer")
            .setContentText(
                "${Format.bytes(task.doneBytes)} / ${Format.bytes(task.totalSize)}" +
                    " · ${Format.rate(task.rateBps)} · 剩余 ${Format.eta(task.etaSecs)}",
            )
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .addAction(0, "取消", cancelIntent)

        if (task.totalSize > 0) {
            // 用 KB 刻度避免超过 Int 上限
            val max = (task.totalSize / 1024).toInt().coerceAtLeast(1)
            val prog = (task.doneBytes / 1024).toInt().coerceIn(0, max)
            builder.setProgress(max, prog, false)
        } else {
            builder.setProgress(0, 0, true)
        }
        return builder.build()
    }

    private fun notifyFinished(task: TaskUi) {
        lastNotifiedAt.remove(task.taskId)
        if (task.state == TaskStates.PAUSED) {
            // 【核心修复】：暂停属于中间挂起态，绝非终态失败，取消正在进行的通知，绝不弹出“接收出错”误导用户
            getSystemService(NotificationManager::class.java).cancel(notifId(task.taskId))
            return
        }
        val direction = if (task.incoming) "接收" else "发送"
        val peer = task.peerName.ifEmpty { task.peerUuid }
        val text = when (task.state) {
            TaskStates.DONE -> "与 $peer：成功 ${task.okFiles} 个，失败 ${task.failedFiles} 个"
            TaskStates.CANCELLED -> "与 $peer 的${direction}已取消"
            TaskStates.REJECTED -> "$peer 拒绝了本次传输"
            else -> "与 $peer 的${direction}出错"
        }
        val notification = NotificationCompat.Builder(this, CHANNEL_DONE_ID)
            .setSmallIcon(
                if (task.state == TaskStates.DONE) android.R.drawable.stat_sys_download_done
                else android.R.drawable.stat_notify_error,
            )
            .setContentTitle("${direction}结束")
            .setContentText(text)
            .setAutoCancel(true)
            .build()
        getSystemService(NotificationManager::class.java)
            .notify(notifId(task.taskId), notification)
    }

    private fun notifId(taskId: Long): Int = ((taskId and 0x7FFFFFFFL).toInt()).coerceAtLeast(1)
}
