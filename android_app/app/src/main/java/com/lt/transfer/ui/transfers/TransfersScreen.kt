package com.lt.transfer.ui.transfers

import android.content.Intent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import com.lt.transfer.engine.LtEngine
import com.lt.transfer.model.TaskStates
import com.lt.transfer.model.TaskUi
import com.lt.transfer.ui.Format
import java.io.File
import java.net.URLConnection

/**
 * 传输页：任务列表（需求 §2.5：实时进度/速率/控制）。
 * 进度数据由 EVT_TASK_PROGRESS 事件驱动，暂停/恢复/取消透传
 * lt_pause_task / lt_resume_task / lt_cancel_task。
 */
@Composable
fun TransfersScreen(modifier: Modifier) {
    val state by LtEngine.uiState.collectAsState()
    val tasks = state.tasks.values.sortedByDescending { it.taskId }

    Column(modifier.fillMaxSize()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("传输任务", style = MaterialTheme.typography.titleMedium)
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.End) {
                TextButton(onClick = { LtEngine.clearRecords() }) { Text("清除记录") }
            }
        }
        LazyColumn(
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(tasks, key = { it.taskId }) { task ->
                TaskCard(task)
            }
            if (tasks.isEmpty()) {
                item {
                    Text(
                        text = "暂无传输任务。\n在「设备」页选择设备即可发送文件；" +
                            "对方发来的文件会在此显示接收进度。",
                        modifier = Modifier.padding(top = 24.dp),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

@Composable
private fun TaskCard(task: TaskUi) {
    val context = LocalContext.current
    val progress = if (task.totalSize > 0) {
        (task.doneBytes.toFloat() / task.totalSize).coerceIn(0f, 1f)
    } else {
        0f
    }
    val dirLabel = if (task.incoming) "↓ 接收" else "↑ 发送"
    val peer = task.peerName.ifEmpty { task.peerUuid }

    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = "$dirLabel · $peer",
                    style = MaterialTheme.typography.titleSmall,
                    modifier = Modifier.weight(1f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (task.transport.isNotEmpty()) {
                    Text(
                        text = task.transport.uppercase(),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(end = 6.dp),
                    )
                }
                Text(
                    text = Format.state(task.state),
                    style = MaterialTheme.typography.bodySmall,
                    color = when (task.state) {
                        TaskStates.DONE -> MaterialTheme.colorScheme.primary
                        TaskStates.ERROR -> MaterialTheme.colorScheme.error
                        TaskStates.CANCELLED, TaskStates.REJECTED ->
                            MaterialTheme.colorScheme.onSurfaceVariant
                        else -> MaterialTheme.colorScheme.tertiary
                    },
                )
            }

            LinearProgressIndicator(
                progress = { progress },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp),
            )

            Text(
                text = "${Format.bytes(task.doneBytes)} / ${Format.bytes(task.totalSize)}" +
                    " · ${Format.rate(task.rateBps)} · 剩余 ${Format.eta(task.etaSecs)}",
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(top = 4.dp),
            )
            if (task.currentFile.isNotEmpty() && TaskStates.isActive(task.state)) {
                Text(
                    text = task.currentFile,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (TaskStates.isTerminal(task.state)) {
                Text(
                    text = "成功 ${task.okFiles} 个，失败 ${task.failedFiles} 个",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            // 控制按钮（取消 / 重试）
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                if (TaskStates.isActive(task.state)) {
                    TextButton(onClick = { LtEngine.cancelTask(task.taskId) }) { Text("取消") }
                }
                if ((task.state == TaskStates.ERROR || task.state == TaskStates.CANCELLED) && !task.incoming) {
                    TextButton(onClick = { LtEngine.resumeTask(task.taskId) }) { Text("重试") }
                }
                if (task.incoming && task.state == TaskStates.DONE && task.currentFile.isNotEmpty()) {
                    TextButton(onClick = { shareReceived(context, task.currentFile) }) {
                        Text("分享文件")
                    }
                    TextButton(onClick = { openReceivedFolder(context) }) {
                        Text("打开文件夹")
                    }
                }
            }
        }
    }
}

/** 接收完成 → 经 FileProvider 分享（文件在应用私有目录 lt/received）。 */
private fun shareReceived(context: android.content.Context, relPath: String) {
    val saveDir = LtEngine.uiState.value.config.saveDir
    val root = if (saveDir.isNotEmpty()) File(saveDir) else File(LtEngine.dataDir(), "received")
    // rel_path 以文件名或 目录/文件 形式给出；取最后一段在落盘目录内定位
    val file = File(root, relPath.substringAfterLast('/'))
    if (!file.exists()) return
    val uri = FileProvider.getUriForFile(
        context,
        "${context.packageName}.fileprovider",
        file,
    )
    val mime = URLConnection.guessContentTypeFromName(file.name) ?: "*/*"
    val intent = Intent(Intent.ACTION_SEND).apply {
        type = mime
        putExtra(Intent.EXTRA_STREAM, uri)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
    context.startActivity(Intent.createChooser(intent, "分享接收的文件"))
}

/**
 * 打开接收目录（系统文件管理器）。Android 无「在文件管理器中打开指定目录」的
 * 标准 API，各厂商接受的 intent 形态不一，按序尝试直至成功：
 * 厂商跳目录私有 action（三星 My Files）→ SAF tree/document URI（目录 MIME）
 * → document/FileProvider URI（resource/folder）。注意不用 *∕* 等宽 MIME，
 * 避免召出网盘类「打开方式」弹窗；全部无处理者时退化为拉起系统文件管理器
 * 首页并 Toast 展示目录位置。
 */
private fun openReceivedFolder(context: android.content.Context) {
    val saveDir = LtEngine.uiState.value.config.saveDir
    val root = if (saveDir.isNotEmpty()) File(saveDir) else File(LtEngine.dataDir(), "received")
    if (!root.exists()) {
        android.widget.Toast
            .makeText(context, "接收目录不存在：${root.path}", android.widget.Toast.LENGTH_LONG)
            .show()
        return
    }
    val external = android.os.Environment.getExternalStorageDirectory()
    val rel = root.relativeToOrNull(external)?.path?.replace('\\', '/')

    val candidates = ArrayList<Intent>()
    // 三星 My Files「跳转目录」私有 action（无文档，部分版本可能失效，失败自动落下一候选）
    candidates += Intent("com.sec.android.app.myfiles.OPEN_FOLDER")
        .putExtra("FOLDER_PATH", root.path)
    if (rel != null) {
        val authority = "com.android.externalstorage.documents"
        val treeUri = android.provider.DocumentsContract.buildTreeDocumentUri(
            authority,
            "primary:$rel",
        )
        val docUri = android.provider.DocumentsContract.buildDocumentUri(
            authority,
            "primary:$rel",
        )
        val dirMime = android.provider.DocumentsContract.Document.MIME_TYPE_DIR
        candidates += Intent(Intent.ACTION_VIEW).setDataAndType(treeUri, dirMime)
        candidates += Intent(Intent.ACTION_VIEW).setDataAndType(docUri, dirMime)
        candidates += Intent(Intent.ACTION_VIEW).setDataAndType(docUri, "resource/folder")
    }
    // FileProvider 形态（应用私有目录与公共存储均在 file_paths 覆盖范围内）
    try {
        val fpUri = FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            root,
        )
        candidates += Intent(Intent.ACTION_VIEW).setDataAndType(fpUri, "resource/folder")
    } catch (_: Exception) {
        // 路径不在 FileProvider 覆盖范围：继续尝试其它候选
    }
    for (intent in candidates) {
        intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        try {
            context.startActivity(intent)
            return
        } catch (_: Exception) {
            // 该形态无可用处理者，尝试下一候选
        }
    }
    // 兜底：拉起系统自带文件管理器首页，Toast 展示接收目录
    val fmPackages = listOf(
        "com.sec.android.app.myfiles", // 三星 My Files
        "com.google.android.documentsui", // Google 文件
        "com.android.documentsui", // AOSP 文件
        "com.mi.android.globalFileexplorer", // 小米文件管理
        "com.huawei.hidisk", // 华为文件管理
        "com.honor.filemanager", // 荣耀文件管理
        "com.coloros.filemanager", // OPPO 文件管理
        "com.vivo.filemanager", // vivo 文件管理
    )
    for (pkg in fmPackages) {
        val launch = context.packageManager.getLaunchIntentForPackage(pkg) ?: continue
        try {
            context.startActivity(launch)
            android.widget.Toast
                .makeText(context, "接收目录：${root.path}", android.widget.Toast.LENGTH_LONG)
                .show()
            return
        } catch (_: Exception) {
            // 该系统文件管理器拉起失败，尝试下一家
        }
    }
    android.widget.Toast
        .makeText(context, "接收目录：${root.path}", android.widget.Toast.LENGTH_LONG)
        .show()
}
