package com.lt.transfer.ui.devices

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContract
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.lt.transfer.engine.LtEngine
import com.lt.transfer.engine.SendStager
import com.lt.transfer.model.ConnectionState
import com.lt.transfer.model.DeviceUi
import com.lt.transfer.ui.Format
import com.lt.transfer.ui.ManualConnectDialog
import kotlinx.coroutines.launch
import java.io.File

/** 支持指定初始定位目录的 SAF 多文件选择器。 */
class PickMultipleDocumentsWithInitialUri : ActivityResultContract<Uri?, List<Uri>>() {
    override fun createIntent(context: Context, input: Uri?): Intent {
        return Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "*/*"
            putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
            if (input != null) {
                putExtra(DocumentsContract.EXTRA_INITIAL_URI, input)
            }
        }
    }

    override fun parseResult(resultCode: Int, intent: Intent?): List<Uri> {
        if (resultCode != Activity.RESULT_OK || intent == null) return emptyList()
        val list = mutableListOf<Uri>()
        intent.data?.let { list.add(it) }
        val clip = intent.clipData
        if (clip != null) {
            for (i in 0 until clip.itemCount) {
                clip.getItemAt(i)?.uri?.let { list.add(it) }
            }
        }
        return list.distinct()
    }
}

/** 根据当前动态 saveDir 计算 com.android.externalstorage.documents 的初始目录 URI。 */
private fun getInitialFolderUri(saveDir: String): Uri? {
    val defaultRoot = File(Environment.getExternalStorageDirectory(), "Download/LocalTransfer")
    val root = if (saveDir.isNotEmpty()) File(saveDir) else defaultRoot
    val external = Environment.getExternalStorageDirectory()
    val rel = root.relativeToOrNull(external)?.path?.replace('\\', '/')
    return if (rel != null) {
        val docId = "primary:$rel"
        DocumentsContract.buildDocumentUri("com.android.externalstorage.documents", docId)
    } else {
        null
    }
}

/**
 * 设备页：局域网在线设备列表（需求 §2.6：设备列表展示 + 刷新扫描），
 * 点击设备行连接/断开，行内按钮直接发起选文件发送。
 */
@Composable
fun DevicesScreen(modifier: Modifier) {
    val state by LtEngine.uiState.collectAsState()
    val scope = rememberCoroutineScope()

    // 发送目标 + 选择器结果处理（选择器在结果回调里才知道目标设备）
    var sendTarget by remember { mutableStateOf<DeviceUi?>(null) }
    var showManualConnect by rememberSaveable { mutableStateOf(false) }

    val context = LocalContext.current
    val pickFiles = rememberLauncherForActivityResult(
        PickMultipleDocumentsWithInitialUri(),
    ) { uris ->
        val target = sendTarget ?: return@rememberLauncherForActivityResult
        if (uris.isEmpty()) return@rememberLauncherForActivityResult
        scope.launch { stageAndSend(context, target, uris = uris) }
    }
    val pickTree = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocumentTree(),
    ) { uri ->
        val target = sendTarget ?: return@rememberLauncherForActivityResult
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch { stageAndSend(context, target, tree = uri) }
    }

    Column(modifier.fillMaxSize()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text("局域网设备", style = MaterialTheme.typography.titleMedium)
                Text(
                    text = when {
                        state.scanning -> "正在扫描局域网设备…"
                        state.devices.isEmpty() -> "正在扫描…"
                        else -> "共发现 ${state.devices.size} 台设备"
                    },
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            TextButton(onClick = { showManualConnect = true }) { Text("手动连接") }
            IconButton(onClick = { LtEngine.probeNetwork() }) {
                if (state.scanning) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(20.dp),
                        strokeWidth = 2.dp,
                    )
                } else {
                    Icon(Icons.Filled.Refresh, contentDescription = "刷新发现")
                }
            }
        }

        // 扫描进度条：发现结果异步到达（数秒），期间给出明确反馈
        if (state.scanning) {
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
        }

        LazyColumn(
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(state.devices, key = { it.uuid }) { device ->
                DeviceCard(
                    device = device,
                    onToggleConnect = {
                        if (device.connState == ConnectionState.Connected) {
                            LtEngine.disconnect(device.uuid)
                        } else {
                            LtEngine.connect(device.uuid)
                        }
                    },
                    onPickFiles = {
                        sendTarget = device
                        val initialUri = getInitialFolderUri(state.config.saveDir)
                        pickFiles.launch(initialUri)
                    },
                    onPickFolder = {
                        sendTarget = device
                        val initialUri = getInitialFolderUri(state.config.saveDir)
                        pickTree.launch(initialUri)
                    },
                )
            }
            if (state.devices.isEmpty()) {
                item {
                    Text(
                        text = "尚未发现设备。\n请确认双方在同一局域网（同 WiFi / 热点），" +
                            "或点右上角「手动连接」直接输入对方 IP。",
                        modifier = Modifier.padding(top = 24.dp),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }

    if (showManualConnect) {
        ManualConnectDialog(onDismiss = { showManualConnect = false })
    }
}

/** SAF 选择结果 → 暂存复制 → lt_send_files（未连接时核心自动先连）。 */
private suspend fun stageAndSend(
    context: android.content.Context,
    target: DeviceUi,
    uris: List<android.net.Uri>? = null,
    tree: android.net.Uri? = null,
) {
    LtEngine.setStaging(true)
    try {
        val staged = when {
            uris != null -> SendStager.stageFiles(context, uris)
            tree != null -> SendStager.stageTree(context, tree)
            else -> null
        }
        if (staged == null) return
        LtEngine.sendFiles(target.uuid, staged.paths, staged.rootDir)
    } catch (_: Exception) {
        // 复制失败（空间不足等）：暂存目录已在内部清理
    } finally {
        LtEngine.setStaging(false)
    }
}

@Composable
private fun DeviceCard(
    device: DeviceUi,
    onToggleConnect: () -> Unit,
    onPickFiles: () -> Unit,
    onPickFolder: () -> Unit,
) {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(
                        text = device.name,
                        style = MaterialTheme.typography.titleSmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = "${Format.deviceType(device.deviceType)} · " +
                            "${device.ip}:${device.quicPort} · ${device.source}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                TextButton(onClick = onToggleConnect) {
                    Text(
                        when (device.connState) {
                            ConnectionState.Connected -> "断开"
                            ConnectionState.Connecting -> "连接中…"
                            ConnectionState.Disconnected -> "重新连接"
                            ConnectionState.None -> "连接"
                        },
                    )
                }
            }
            if (device.connState == ConnectionState.Connected && device.transport.isNotEmpty()) {
                Text(
                    text = "已连接（${device.transport.uppercase()}）",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.primary,
                )
            }
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(top = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedButton(onClick = onPickFiles, modifier = Modifier.weight(1f)) {
                    Text("发送文件")
                }
                OutlinedButton(onClick = onPickFolder, modifier = Modifier.weight(1f)) {
                    Text("发送文件夹")
                }
            }
        }
    }
}
