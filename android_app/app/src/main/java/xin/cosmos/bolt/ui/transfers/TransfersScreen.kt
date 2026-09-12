package xin.cosmos.bolt.ui.transfers

import android.app.DownloadManager
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.media.MediaScannerConnection
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import android.widget.Toast
import androidx.compose.foundation.clickable
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
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import xin.cosmos.bolt.ui.components.card.BtCard
import xin.cosmos.bolt.ui.components.dialogs.BtConfirmDialog
import xin.cosmos.bolt.ui.components.feedback.EmptyStateView
import xin.cosmos.bolt.ui.components.feedback.SmoothProgressBar
import xin.cosmos.bolt.ui.components.feedback.StatusBadge
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import xin.cosmos.bolt.R
import xin.cosmos.bolt.engine.BtEngine
import xin.cosmos.bolt.model.TaskStates
import xin.cosmos.bolt.model.TaskUi
import xin.cosmos.bolt.ui.Format
import java.io.File
import java.net.URLConnection

/**
 * 传输页：任务列表（需求 §2.5：实时进度/速率/控制）。
 * 进度数据由 EVT_TASK_PROGRESS 事件驱动，暂停/恢复/取消透传
 * bt_pause_task / bt_resume_task / bt_cancel_task。
 */
@Composable
fun TransfersScreen(modifier: Modifier) {
    val state by BtEngine.uiState.collectAsState()
    val tasks = state.tasks.values.sortedByDescending { it.taskId }
    var showClearConfirm by remember { mutableStateOf(false) }

    Column(modifier.fillMaxSize()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(stringResource(R.string.transfers_title), style = MaterialTheme.typography.titleMedium)
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.End) {
                if (tasks.isNotEmpty()) {
                    TextButton(onClick = { showClearConfirm = true }) {
                        Text(stringResource(R.string.transfers_btn_clear_records))
                    }
                }
            }
        }
        LazyColumn(
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(tasks, key = { it.taskId }) { task ->
                TaskCard(task = task, modifier = Modifier.animateItem())
            }
            if (tasks.isEmpty()) {
                item {
                    EmptyStateView(
                        icon = rememberVectorPainter(Icons.AutoMirrored.Filled.List),
                        title = stringResource(R.string.tab_transfers),
                        description = stringResource(R.string.transfers_empty_hint),
                    )
                }
            }
        }
    }

    if (showClearConfirm) {
        BtConfirmDialog(
            title = stringResource(R.string.transfers_btn_clear_records),
            message = stringResource(R.string.transfers_confirm_clear_msg),
            isDestructive = true,
            onConfirm = {
                showClearConfirm = false
                BtEngine.clearRecords()
            },
            onDismiss = { showClearConfirm = false },
        )
    }
}

@Composable
private fun TaskCard(task: TaskUi, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val progress = if (task.totalSize > 0) {
        (task.doneBytes.toFloat() / task.totalSize).coerceIn(0f, 1f)
    } else {
        0f
    }
    val dirLabel = if (task.incoming) stringResource(R.string.transfers_dir_incoming) else stringResource(R.string.transfers_dir_outgoing)
    val peer = task.peerName.ifEmpty { task.peerUuid }

    val isIncomingDone = task.incoming && task.state == TaskStates.DONE && task.currentFile.isNotEmpty()

    BtCard(
        modifier = modifier,
        onClick = if (isIncomingDone) { { openReceivedFile(context, task.currentFile) } } else null,
    ) {
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
                val stateResId = Format.stateResId(task.state)
                val stateText = if (stateResId != 0) stringResource(stateResId) else task.state
                val stateColor = when (task.state) {
                    TaskStates.DONE -> MaterialTheme.colorScheme.primary
                    TaskStates.ERROR -> MaterialTheme.colorScheme.error
                    TaskStates.CANCELLED, TaskStates.REJECTED -> MaterialTheme.colorScheme.onSurfaceVariant
                    else -> MaterialTheme.colorScheme.tertiary
                }
                StatusBadge(
                    text = stateText,
                    color = stateColor,
                    hasDot = !TaskStates.isTerminal(task.state),
                )
            }

            SmoothProgressBar(
                progress = progress,
                modifier = Modifier.padding(top = 8.dp),
            )

            if (!TaskStates.isTerminal(task.state)) {
                Text(
                    text = stringResource(
                        R.string.transfers_active_progress_format,
                        Format.bytes(task.doneBytes),
                        Format.bytes(task.totalSize),
                        Format.rate(task.rateBps),
                        Format.eta(task.etaSecs),
                    ),
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }
            if (task.currentFile.isNotEmpty()) {
                Text(
                    text = task.currentFile,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (TaskStates.isTerminal(task.state)) {
                val effectiveAvgRate = if (task.avgRateBps > 0) {
                    task.avgRateBps
                } else if (task.durationMs > 0 && task.totalSize > 0) {
                    (task.totalSize * 1000) / task.durationMs
                } else {
                    task.rateBps
                }
                Text(
                    text = stringResource(
                        R.string.transfers_summary_format,
                        Format.bytes(task.totalSize),
                        Format.rate(effectiveAvgRate),
                        task.okFiles,
                        task.failedFiles,
                    ),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }

            // 控制按钮（取消 / 打开文件 / 打开文件夹 / 分享）
            Row(
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.padding(top = 6.dp),
            ) {
                if (TaskStates.isActive(task.state)) {
                    TextButton(onClick = { BtEngine.cancelTask(task.taskId) }) { Text(stringResource(R.string.common_cancel)) }
                }
                if (isIncomingDone) {
                    FilledTonalButton(
                        onClick = { openReceivedFile(context, task.currentFile) },
                        contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                    ) {
                        Text(stringResource(R.string.transfers_btn_open_file))
                    }
                    OutlinedButton(
                        onClick = { openReceivedFolder(context) },
                        contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                    ) {
                        Text(stringResource(R.string.transfers_btn_open_folder))
                    }
                    TextButton(
                        onClick = { shareReceived(context, task.currentFile) },
                        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                    ) {
                        Text(stringResource(R.string.transfers_btn_share))
                    }
                }
            }
        }
    }
}

/** 在落盘目录内定位接收的文件（优先相对路径直查，回落单文件名匹配）。 */
private fun resolveReceivedFile(relPath: String): File? {
    val saveDir = BtEngine.uiState.value.config.saveDir
    val defaultRoot = File(Environment.getExternalStorageDirectory(), "Download/Bolt")
    val root = if (saveDir.isNotEmpty()) File(saveDir) else defaultRoot
    if (!root.exists()) return null
    val direct = File(root, relPath)
    if (direct.exists()) return direct
    val byName = File(root, relPath.substringAfterLast('/'))
    if (byName.exists()) return byName
    return null
}

/** 调用系统关联应用直接打开接收的文件。 */
private fun openReceivedFile(context: Context, relPath: String) {
    val file = resolveReceivedFile(relPath)
    if (file == null || !file.exists()) {
        Toast.makeText(context, context.getString(R.string.transfers_toast_file_not_found), Toast.LENGTH_SHORT).show()
        return
    }

    // 触发系统媒体库扫描，保证外部查看器能立刻索引到最新落盘文件
    try {
        MediaScannerConnection.scanFile(
            context,
            arrayOf(file.absolutePath),
            null,
            null,
        )
    } catch (_: Exception) {}

    val uri = try {
        FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            file,
        )
    } catch (e: Exception) {
        Toast.makeText(context, context.getString(R.string.transfers_toast_get_file_perm_failed, e.message ?: ""), Toast.LENGTH_SHORT).show()
        return
    }

    val mime = URLConnection.guessContentTypeFromName(file.name) ?: "*/*"
    val intent = Intent(Intent.ACTION_VIEW).apply {
        setDataAndType(uri, mime)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }

    try {
        context.startActivity(Intent.createChooser(intent, context.getString(R.string.transfers_chooser_open_file, file.name)))
    } catch (_: Exception) {
        try {
            context.startActivity(intent)
        } catch (_: Exception) {
            Toast.makeText(context, context.getString(R.string.transfers_toast_no_app_to_open, file.extension), Toast.LENGTH_LONG).show()
        }
    }
}

/** 接收完成 → 经 FileProvider 分享。 */
private fun shareReceived(context: Context, relPath: String) {
    val file = resolveReceivedFile(relPath)
    if (file == null || !file.exists()) {
        Toast.makeText(context, context.getString(R.string.transfers_toast_file_not_found), Toast.LENGTH_SHORT).show()
        return
    }
    val uri = try {
        FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            file,
        )
    } catch (e: Exception) {
        Toast.makeText(context, context.getString(R.string.transfers_toast_get_share_perm_failed, e.message ?: ""), Toast.LENGTH_SHORT).show()
        return
    }
    val mime = URLConnection.guessContentTypeFromName(file.name) ?: "*/*"
    val intent = Intent(Intent.ACTION_SEND).apply {
        type = mime
        putExtra(Intent.EXTRA_STREAM, uri)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
    context.startActivity(Intent.createChooser(intent, context.getString(R.string.transfers_chooser_share_file)))
}

/**
 * 打开接收目录（系统/厂商文件管理器）。
 *
 * 采用多层级精准直入策略：
 * 1. 优先直达 Google/AOSP 原生系统文件管理器（com.google.android.documentsui / com.android.documentsui），
 *    传入精准 documentUri（如 primary:Download/Bolt），100% 直入该目标子目录。
 * 2. SAF 树文档通用协议（多应用自由解析）
 * 3. FileProvider 目录暴露
 * 4. 三星 My Files 专有动作（旧系统兼容）
 * 5. 系统下载管理器入口（仅在位于 Download 且前述均不可用时作为兜底）
 * 6. 兜底：复制目录路径到剪贴板并 Toast 友好提醒
 */
private fun openReceivedFolder(context: Context) {
    val saveDir = BtEngine.uiState.value.config.saveDir
    val defaultRoot = File(Environment.getExternalStorageDirectory(), "Download/Bolt")
    val root = if (saveDir.isNotEmpty()) File(saveDir) else defaultRoot
    if (!root.exists()) {
        root.mkdirs()
    }
    if (!root.exists()) {
        Toast.makeText(context, context.getString(R.string.transfers_toast_dir_not_exist, root.path), Toast.LENGTH_LONG).show()
        return
    }

    val external = Environment.getExternalStorageDirectory()
    val rel = root.relativeToOrNull(external)?.path?.replace('\\', '/')

    val candidates = ArrayList<Intent>()

    if (rel != null) {
        val docId = "primary:$rel"
        val docUri = Uri.parse("content://com.android.externalstorage.documents/document/" + Uri.encode(docId))
        val treeUri = Uri.parse("content://com.android.externalstorage.documents/tree/" + Uri.encode(docId))
        val dirMime = DocumentsContract.Document.MIME_TYPE_DIR

        // 1. 优先直调 Android 原生系统文件管理器（直入该层级目录）
        candidates += Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(docUri, dirMime)
            setPackage("com.google.android.documentsui")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        candidates += Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(docUri, dirMime)
            setPackage("com.android.documentsui")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        }

        // 2. 通用 SAF Document 协议
        candidates += Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(docUri, dirMime)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        candidates += Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(treeUri, dirMime)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        }
    }

    // 3. FileProvider 目录关联
    try {
        val fpUri = FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            root,
        )
        candidates += Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(fpUri, "vnd.android.document/directory")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        candidates += Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(fpUri, "resource/folder")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        }
    } catch (_: Exception) {}

    // 4. 三星「我的文件」专有动作
    candidates += Intent("com.sec.android.app.myfiles.OPEN_FOLDER")
        .putExtra("FOLDER_PATH", root.absolutePath)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

    // 5. 若在系统 Download 目录内，尝试调起系统下载管理器（最后兜底）
    if (rel != null && rel.startsWith("Download", ignoreCase = true)) {
        candidates += Intent(DownloadManager.ACTION_VIEW_DOWNLOADS).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
    }

    for (intent in candidates) {
        try {
            context.startActivity(intent)
            return
        } catch (_: Exception) {
            // 尝试下一形态
        }
    }

    // 5. 各厂商原生文件管理器（已在 AndroidManifest.xml queries 中声明）
    val fmPackages = listOf(
        "com.android.fileexplorer",          // 小米 / 红米 MIUI & HyperOS
        "com.mi.android.globalFileexplorer", // 小米国际版
        "com.huawei.filemanager",            // 华为 HarmonyOS & EMUI
        "com.huawei.hidisk",                 // 华为备选
        "com.hihonor.filemanager",           // 荣耀 MagicOS
        "com.honor.filemanager",             // 荣耀备选
        "com.coloros.filemanager",           // OPPO / 一加 / realme (ColorOS)
        "com.oneplus.filemanager",           // 一加氢OS备选
        "com.vivo.filemanager",              // vivo / iQOO (OriginOS / Funtouch)
        "com.sec.android.app.myfiles",       // 三星 OneUI
        "com.google.android.apps.nbu.files", // Files by Google
        "com.google.android.documentsui",    // 谷歌原生 Files
        "com.android.documentsui",           // AOSP Files
    )

    for (pkg in fmPackages) {
        val launch = context.packageManager.getLaunchIntentForPackage(pkg) ?: continue
        launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        launch.putExtra("current_directory", root.absolutePath)
        launch.putExtra("root_path", root.absolutePath)
        launch.putExtra("path", root.absolutePath)
        try {
            context.startActivity(launch)
            Toast.makeText(context, context.getString(R.string.transfers_toast_opened_file_manager, root.path), Toast.LENGTH_LONG).show()
            return
        } catch (_: Exception) {}
    }

    // 6. 兜底：复制路径至剪贴板并友好提示
    try {
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.setPrimaryClip(ClipData.newPlainText("Bolt Save Dir", root.absolutePath))
        Toast.makeText(context, context.getString(R.string.transfers_toast_copied_path_to_clipboard, root.path), Toast.LENGTH_LONG).show()
    } catch (_: Exception) {
        Toast.makeText(context, context.getString(R.string.transfers_toast_save_dir_path, root.path), Toast.LENGTH_LONG).show()
    }
}
