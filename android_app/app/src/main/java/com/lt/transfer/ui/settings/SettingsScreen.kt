package com.lt.transfer.ui.settings

import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.lt.transfer.engine.LtEngine
import com.lt.transfer.model.UiState
import org.json.JSONObject
import java.io.File

/**
 * 设置页：全部为 `lt_set_config` / `lt_clear_*` 的透传（需求 §2.6：
 * 明文/加密与保存目录等由核心配置驱动，UI 不实现业务）。
 * 与电脑端设置项对齐：网络传输（QUIC 优先/并发/分片）、文件接收
 * （保存目录/冲突策略）、设备发现（mDNS/广播端口）、本机设备
 * （名称/隐身/自动接收）。
 */
@Composable
fun SettingsScreen(modifier: Modifier) {
    val state by LtEngine.uiState.collectAsState()
    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("设置", style = MaterialTheme.typography.titleMedium)

        DeviceNameSection(state)
        HorizontalDivider()
        NetworkSection(state)
        HorizontalDivider()
        ReceiveSection(state)
        HorizontalDivider()
        DiscoverySection(state)
        HorizontalDivider()
        SwitchRow(
            title = "隐身模式",
            subtitle = "不在对方设备列表中广播本机（仍可被手动 IP 连接）",
            checked = state.config.stealthMode,
            onCheckedChange = {
                LtEngine.setConfig(JSONObject().put("stealth_mode", it))
            },
        )
        SwitchRow(
            title = "自动接收已信任设备的文件",
            subtitle = "配对过的设备发来文件时免确认直接接收",
            checked = state.config.autoAcceptTrusted,
            onCheckedChange = {
                LtEngine.setConfig(JSONObject().put("auto_accept_trusted", it))
            },
        )
        HorizontalDivider()

        InfoRow("本机指纹", state.fingerprint.ifEmpty { "—" }, mono = true)
        InfoRow("核心版本", state.version.ifEmpty { "—" })

        HorizontalDivider()
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = { LtEngine.clearRecords() }) { Text("清除传输记录") }
            OutlinedButton(onClick = { LtEngine.clearTempCache() }) { Text("清理临时缓存") }
        }
        Text(
            text = "清理临时缓存会删除未完成的接收文件与断点续传进度。",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = "设置保存后立即生效，不影响正在传输中的任务；广播端口变更会在当前任务结束后生效。",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun DeviceNameSection(state: UiState) {
    var name by rememberSaveable(state.config.deviceName) {
        mutableStateOf(state.config.deviceName)
    }
    Column {
        OutlinedTextField(
            value = name,
            onValueChange = { name = it },
            label = { Text("设备名称（对方看到的名字）") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            TextButton(
                enabled = name.isNotBlank() && name != state.config.deviceName,
                onClick = {
                    LtEngine.setConfig(JSONObject().put("device_name", name.trim()))
                },
            ) { Text("保存") }
        }
        // 本机网络信息：多网卡时全部展示，端口为端口池避让后的实际监听值
        InfoRow("本机 IP", state.localIps.joinToString("、").ifEmpty { "—" }, mono = true)
        InfoRow("监听端口（实际）", if (state.localPort > 0) state.localPort.toString() else "—")
    }
}

/** 网络传输：传输协议（QUIC/TCP 二选一）/ 并发流数 / 分片大小（与电脑端对齐）。 */
@Composable
private fun NetworkSection(state: UiState) {
    var concurrency by rememberSaveable(state.config.concurrency) {
        mutableStateOf(if (state.config.concurrency > 0) state.config.concurrency.toString() else "4")
    }
    var protoMenuOpen by remember { mutableStateOf(false) }
    var chunkMenuOpen by remember { mutableStateOf(false) }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("网络传输", style = MaterialTheme.typography.titleSmall)
        Box {
            OutlinedButton(onClick = { protoMenuOpen = true }) {
                Text("传输协议：${if (state.config.preferQuic) "QUIC（默认）" else "TCP"}")
            }
            DropdownMenu(expanded = protoMenuOpen, onDismissRequest = { protoMenuOpen = false }) {
                DropdownMenuItem(
                    text = { Text("QUIC（默认）") },
                    onClick = {
                        protoMenuOpen = false
                        if (!state.config.preferQuic) {
                            LtEngine.setConfig(JSONObject().put("prefer_quic", true))
                        }
                    },
                )
                DropdownMenuItem(
                    text = { Text("TCP") },
                    onClick = {
                        protoMenuOpen = false
                        if (state.config.preferQuic) {
                            LtEngine.setConfig(JSONObject().put("prefer_quic", false))
                        }
                    },
                )
            }
        }
        Text(
            text = "两端任一设备选择 TCP，连接即使用 TCP；两端都选 QUIC 才用 QUIC。已建立的连接仍使用原协议；所选协议不可用时直接提示连接失败，不自动切换。",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = concurrency,
                onValueChange = { concurrency = it.filter(Char::isDigit).take(2) },
                label = { Text("并发流数量（1-16）") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                modifier = Modifier.weight(1f),
            )
            TextButton(
                enabled = concurrency.toIntOrNull() != null &&
                        concurrency.toInt() != state.config.concurrency,
                onClick = {
                    val c = (concurrency.toIntOrNull() ?: 4).coerceIn(1, 16)
                    LtEngine.setConfig(JSONObject().put("concurrency", c))
                },
            ) { Text("保存") }
        }
        Box {
            OutlinedButton(onClick = { chunkMenuOpen = true }) {
                Text("分片大小：${chunkLabel(state.config.chunkSize)}")
            }
            DropdownMenu(expanded = chunkMenuOpen, onDismissRequest = { chunkMenuOpen = false }) {
                for ((size, label) in CHUNK_OPTIONS) {
                    DropdownMenuItem(
                        text = { Text(label) },
                        onClick = {
                            chunkMenuOpen = false
                            if (size != state.config.chunkSize) {
                                LtEngine.setConfig(JSONObject().put("chunk_size", size))
                            }
                        },
                    )
                }
            }
        }
    }
}

/** 解析 SAF DocumentTree Uri 为设备物理路径。 */
private fun resolveTreeUriToPath(uri: Uri): String? {
    try {
        val docId = DocumentsContract.getTreeDocumentId(uri) ?: return null
        if (docId.startsWith("primary:", ignoreCase = true)) {
            val rel = docId.substringAfter("primary:").trimStart('/', '\\')
            val base = Environment.getExternalStorageDirectory()
            return if (rel.isEmpty()) base.absolutePath else File(base, rel).absolutePath
        } else {
            val parts = docId.split(":")
            if (parts.size >= 2) {
                val storageId = parts[0]
                val rel = parts[1].trimStart('/', '\\')
                val candidate1 = File("/storage/$storageId", rel)
                if (candidate1.exists() || candidate1.parentFile?.exists() == true) {
                    return candidate1.absolutePath
                }
                val candidate2 = File("/mnt/media_rw/$storageId", rel)
                if (candidate2.exists() || candidate2.parentFile?.exists() == true) {
                    return candidate2.absolutePath
                }
                return candidate1.absolutePath
            }
        }
    } catch (_: Exception) {
    }
    return null
}

/** 文件接收：选择文件夹（末级目录固定为 /LocalTransfer） / 同名冲突策略。 */
@Composable
private fun ReceiveSection(state: UiState) {
    val context = LocalContext.current
    val currentSaveDir = state.config.saveDir.ifEmpty {
        File(Environment.getExternalStorageDirectory(), "Download/LocalTransfer").absolutePath
    }

    val folderPicker = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree(),
    ) { uri: Uri? ->
        if (uri != null) {
            try {
                context.contentResolver.takePersistableUriPermission(
                    uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION,
                )
            } catch (_: Exception) {
            }

            val parentPath = resolveTreeUriToPath(uri)
            if (!parentPath.isNullOrEmpty()) {
                val cleanParent = parentPath.trimEnd('/', '\\')
                val finalDir = if (cleanParent.endsWith("/LocalTransfer", ignoreCase = true) ||
                    cleanParent.endsWith("\\LocalTransfer", ignoreCase = true)
                ) {
                    cleanParent
                } else {
                    "$cleanParent/LocalTransfer"
                }

                val dir = File(finalDir)
                if (!dir.exists()) {
                    dir.mkdirs()
                }
                LtEngine.setConfig(JSONObject().put("save_dir", finalDir))
                Toast.makeText(context, "接收目录已更新为：$finalDir", Toast.LENGTH_SHORT).show()
            } else {
                Toast.makeText(context, "未能识别该目录，请选择内部存储中的有效文件夹", Toast.LENGTH_LONG).show()
            }
        }
    }

    var collisionMenuOpen by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("文件接收", style = MaterialTheme.typography.titleSmall)

        // 完整接收路径展示卡片与选择操作
        Card(
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
            ),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(
                modifier = Modifier.padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(
                    text = "文件下载路径",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.primary,
                )
                Text(
                    text = currentSaveDir,
                    style = MaterialTheme.typography.bodyMedium,
                    fontFamily = FontFamily.Monospace,
                    modifier = Modifier.padding(vertical = 2.dp),
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End,
                ) {
                    OutlinedButton(
                        onClick = { folderPicker.launch(null) },
                    ) {
                        Text("选择文件夹")
                    }
                }
            }
        }

        Box {
            OutlinedButton(onClick = { collisionMenuOpen = true }) {
                Text("同名文件：${if (state.config.collision == "overwrite") "直接覆盖" else "自动重命名"}")
            }
            DropdownMenu(expanded = collisionMenuOpen, onDismissRequest = { collisionMenuOpen = false }) {
                DropdownMenuItem(
                    text = { Text("自动重命名") },
                    onClick = {
                        collisionMenuOpen = false
                        if (state.config.collision != "rename") {
                            LtEngine.setConfig(JSONObject().put("collision", "rename"))
                        }
                    },
                )
                DropdownMenuItem(
                    text = { Text("直接覆盖") },
                    onClick = {
                        collisionMenuOpen = false
                        if (state.config.collision != "overwrite") {
                            LtEngine.setConfig(JSONObject().put("collision", "overwrite"))
                        }
                    },
                )
            }
        }
    }
}

/** 设备发现：mDNS 开关 / 广播端口（与电脑端对齐）。 */
@Composable
private fun DiscoverySection(state: UiState) {
    var port by rememberSaveable(state.config.listenPort) {
        mutableStateOf(if (state.config.listenPort > 0) state.config.listenPort.toString() else "8899")
    }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("设备发现", style = MaterialTheme.typography.titleSmall)
        SwitchRow(
            title = "mDNS 自动发现",
            subtitle = "局域网内自动广播与发现设备",
            checked = state.config.useMdns,
            onCheckedChange = {
                LtEngine.setConfig(JSONObject().put("use_mdns", it))
            },
        )
        Row(verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = port,
                onValueChange = { port = it.filter(Char::isDigit).take(5) },
                label = { Text("广播端口") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                modifier = Modifier.weight(1f),
            )
            TextButton(
                enabled = port.toIntOrNull() != null &&
                        port.toInt() != state.config.listenPort,
                onClick = {
                    val p = (port.toIntOrNull() ?: 8899).coerceIn(1, 65535)
                    LtEngine.setConfig(JSONObject().put("listen_port", p))
                },
            ) { Text("保存") }
        }
    }
}

/** 分片大小选项（与电脑端设置一致）。 */
private val CHUNK_OPTIONS = listOf(
    256L * 1024 to "256KB",
    512L * 1024 to "512KB",
    1024L * 1024 to "1MB",
    4L * 1024 * 1024 to "4MB",
)

private fun chunkLabel(size: Long): String =
    CHUNK_OPTIONS.firstOrNull { it.first == size }?.second ?: "${size / 1024}KB"

@Composable
private fun SwitchRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            Text(
                subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}

@Composable
private fun InfoRow(label: String, value: String, mono: Boolean = false) {
    Column {
        Text(
            label,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            value,
            style = MaterialTheme.typography.bodyMedium,
            fontFamily = if (mono) FontFamily.Monospace else FontFamily.Default,
        )
    }
}
