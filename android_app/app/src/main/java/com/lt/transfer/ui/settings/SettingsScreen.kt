package com.lt.transfer.ui.settings

import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.lt.transfer.R
import com.lt.transfer.engine.LtEngine
import com.lt.transfer.model.UiState
import org.json.JSONObject
import java.io.File

/**
 * 设置页（高质感立体优化版）：
 * 1. 参数名称与参数值严格保持在同一行（左侧标题，右侧参数值右对齐）；补充描述单独在第二行另起一行。
 * 2. 参数值禁止截断省略，若超过宽度自适应折行完整呈现。
 * 3. 设备安全指纹默认隐藏，带小眼睛切换明文与密文，支持一键复制。
 * 4. 视觉体验：干净、清爽、立体质感，模块立体微阴影与高科技感色标。
 */
@Composable
fun SettingsScreen(modifier: Modifier) {
    val state by LtEngine.uiState.collectAsState()

    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        DeviceModule(state)
        NetworkModule(state)
        ReceiveModule(state)
        DiscoveryModule(state)
        MaintenanceModule(state)
        AboutModule(state)

        Spacer(Modifier.height(16.dp))
    }
}

// ---------------- 1. 本机设备模块 ----------------

@Composable
private fun DeviceModule(state: UiState) {
    var showNameDialog by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SectionHeader("本机设备")
        SettingCard {
            SettingRow(
                title = "设备名称",
                subtitle = "局域网内其他设备发现与展示的名称",
                onClick = { showNameDialog = true },
            ) {
                ValueBadge(
                    text = "${state.config.deviceName.ifEmpty { "未命名设备" }}  ✎",
                    accent = true,
                    onClick = { showNameDialog = true },
                )
            }

            SettingDivider()

            val ipList = state.localIps.ifEmpty { listOf("—") }
            val ipText = ipList.joinToString("、")
            SettingRow(
                title = "本机 IP",
                subtitle = "当前连接的局域网物理网络地址（支持多网卡/热点）",
                alignTop = ipList.size > 1,
            ) {
                ValueBadge(
                    text = ipText,
                    mono = true,
                )
            }

            SettingDivider()

            SettingRow(
                title = "实际监听端口",
                subtitle = "传输核心实际绑定的本地监听端口",
            ) {
                ValueBadge(
                    text = if (state.localPort > 0) state.localPort.toString() else "—",
                    mono = true,
                )
            }

            SettingDivider()

            SettingRow(
                title = "隐身模式",
                subtitle = "不在对方设备列表中广播本机，仍可被手动输入 IP 连接",
            ) {
                Switch(
                    checked = state.config.stealthMode,
                    onCheckedChange = {
                        LtEngine.setConfig(JSONObject().put("stealth_mode", it))
                    },
                )
            }
        }
    }

    if (showNameDialog) {
        EditNameDialog(
            current = state.config.deviceName,
            onDismiss = { showNameDialog = false },
            onConfirm = { newName ->
                showNameDialog = false
                if (newName.isNotBlank() && newName != state.config.deviceName) {
                    LtEngine.setConfig(JSONObject().put("device_name", newName.trim()))
                }
            },
        )
    }
}

// ---------------- 2. 网络传输模块 ----------------

@Composable
private fun NetworkModule(state: UiState) {
    var protoMenuOpen by remember { mutableStateOf(false) }
    var chunkMenuOpen by remember { mutableStateOf(false) }
    var concurrencyMenuOpen by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SectionHeader("网络传输")
        SettingCard {
            // 传输协议
            SettingRow(
                title = "传输协议",
                subtitle = "两端任一设备选择 TCP 即使用 TCP，均选 QUIC 才用 QUIC",
                onClick = { protoMenuOpen = true },
            ) {
                Box {
                    ValueBadge(
                        text = if (state.config.preferQuic) "QUIC（推荐） ▾" else "TCP ▾",
                        accent = state.config.preferQuic,
                        onClick = { protoMenuOpen = true },
                    )
                    DropdownMenu(
                        expanded = protoMenuOpen,
                        onDismissRequest = { protoMenuOpen = false },
                    ) {
                        DropdownMenuItem(
                            text = { Text("QUIC（推荐）") },
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
            }

            SettingDivider()

            // 并发流数量
            SettingRow(
                title = "并发流数量",
                subtitle = "批量文件传输时的并行流数量（推荐 4）",
                onClick = { concurrencyMenuOpen = true },
            ) {
                Box {
                    val currentC = if (state.config.concurrency > 0) state.config.concurrency else 4
                    ValueBadge(
                        text = "$currentC 个流 ▾",
                        onClick = { concurrencyMenuOpen = true },
                    )
                    DropdownMenu(
                        expanded = concurrencyMenuOpen,
                        onDismissRequest = { concurrencyMenuOpen = false },
                    ) {
                        listOf(1, 2, 4, 8, 12, 16).forEach { num ->
                            DropdownMenuItem(
                                text = { Text("$num 个并发流${if (num == 4) "（推荐）" else ""}") },
                                onClick = {
                                    concurrencyMenuOpen = false
                                    if (num != state.config.concurrency) {
                                        LtEngine.setConfig(JSONObject().put("concurrency", num))
                                    }
                                },
                            )
                        }
                    }
                }
            }

            SettingDivider()

            // 分片大小
            SettingRow(
                title = "分片大小",
                subtitle = "单次传输拆包尺寸，影响吞吐与内存负载",
                onClick = { chunkMenuOpen = true },
            ) {
                Box {
                    ValueBadge(
                        text = "${chunkLabel(state.config.chunkSize)} ▾",
                        onClick = { chunkMenuOpen = true },
                    )
                    DropdownMenu(
                        expanded = chunkMenuOpen,
                        onDismissRequest = { chunkMenuOpen = false },
                    ) {
                        CHUNK_OPTIONS.forEach { (size, label) ->
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
    }
}

// ---------------- 3. 文件接收模块 ----------------

@Composable
private fun ReceiveModule(state: UiState) {
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

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SectionHeader("文件接收")
        SettingCard {
            // 接收保存目录：参数名称与参数值同一行，参数值超宽自动折行不截断；补充说明另起一行
            val safeWrappedPath = currentSaveDir.replace("/", "/\u200B")
            SettingRow(
                title = "保存目录",
                subtitle = "选择的文件夹为父目录，末级目录自动固定为 /LocalTransfer",
                alignTop = true,
                onClick = { folderPicker.launch(null) },
            ) {
                ValueBadge(
                    text = "$safeWrappedPath  📁 更改",
                    mono = true,
                    accent = true,
                    onClick = { folderPicker.launch(null) },
                )
            }

            SettingDivider()

            // 同名文件冲突策略
            SettingRow(
                title = "同名冲突策略",
                subtitle = "目标目录中已存在同名文件时的应对方式",
                onClick = { collisionMenuOpen = true },
            ) {
                Box {
                    ValueBadge(
                        text = "${if (state.config.collision == "overwrite") "直接覆盖" else "自动重命名"} ▾",
                        onClick = { collisionMenuOpen = true },
                    )
                    DropdownMenu(
                        expanded = collisionMenuOpen,
                        onDismissRequest = { collisionMenuOpen = false },
                    ) {
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

            SettingDivider()

            // 自动接收已信任设备
            SettingRow(
                title = "自动接收信任文件",
                subtitle = "已完成配对的信任设备发送文件时，免确认直接开始接收",
            ) {
                Switch(
                    checked = state.config.autoAcceptTrusted,
                    onCheckedChange = {
                        LtEngine.setConfig(JSONObject().put("auto_accept_trusted", it))
                    },
                )
            }
        }
    }
}

// ---------------- 4. 设备发现模块 ----------------

@Composable
private fun DiscoveryModule(state: UiState) {
    var showPortDialog by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SectionHeader("设备发现")
        SettingCard {
            SettingRow(
                title = "mDNS 自动发现",
                subtitle = "局域网内通过多播 DNS 自动广播与发现设备",
            ) {
                Switch(
                    checked = state.config.useMdns,
                    onCheckedChange = {
                        LtEngine.setConfig(JSONObject().put("use_mdns", it))
                    },
                )
            }

            SettingDivider()

            SettingRow(
                title = "广播探测端口",
                subtitle = "UDP 广播探测端口，修改后在当前任务结束后生效",
                onClick = { showPortDialog = true },
            ) {
                ValueBadge(
                    text = "${if (state.config.listenPort > 0) state.config.listenPort else 8899}  ✎",
                    mono = true,
                    accent = true,
                    onClick = { showPortDialog = true },
                )
            }
        }
    }

    if (showPortDialog) {
        EditPortDialog(
            current = if (state.config.listenPort > 0) state.config.listenPort else 8899,
            onDismiss = { showPortDialog = false },
            onConfirm = { newPort ->
                showPortDialog = false
                if (newPort in 1..65535 && newPort != state.config.listenPort) {
                    LtEngine.setConfig(JSONObject().put("listen_port", newPort))
                }
            },
        )
    }
}

// ---------------- 5. 存储与维护模块 ----------------

@Composable
private fun MaintenanceModule(state: UiState) {
    val context = LocalContext.current

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SectionHeader("存储与维护")
        SettingCard {
            SettingRow(
                title = "传输任务记录",
                subtitle = "清理列表中所有已完成、失败或取消的历史记录",
            ) {
                OutlinedButton(
                    shape = RoundedCornerShape(10.dp),
                    onClick = {
                        LtEngine.clearRecords()
                        Toast.makeText(context, "传输记录已清空", Toast.LENGTH_SHORT).show()
                    },
                ) {
                    Text("全部清除")
                }
            }

            SettingDivider()

            SettingRow(
                title = "传输临时缓存",
                subtitle = "删除未完成的接收碎片与断点续传临时缓存文件",
            ) {
                OutlinedButton(
                    shape = RoundedCornerShape(10.dp),
                    onClick = {
                        LtEngine.clearTempCache()
                        Toast.makeText(context, "临时缓存已清理", Toast.LENGTH_SHORT).show()
                    },
                ) {
                    Text("清理缓存")
                }
            }
        }
    }
}

// ---------------- 6. 关于本机模块 ----------------

@Composable
private fun AboutModule(state: UiState) {
    val context = LocalContext.current
    val clipboardManager = LocalClipboardManager.current
    var fingerprintVisible by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SectionHeader("关于本机")
        SettingCard {
            val fp = state.fingerprint.ifEmpty { "—" }
            val maskedFp = "••••••••••••"
            val displayFp = if (fingerprintVisible) {
                fp.replace(":", ":\u200B")
            } else {
                maskedFp
            }

            SettingRow(
                title = "设备安全指纹",
                subtitle = "用于身份配对校验的 BLAKE3 安全标识（点击复制，默认隐藏）",
                alignTop = fingerprintVisible,
                onClick = {
                    if (state.fingerprint.isNotEmpty()) {
                        clipboardManager.setText(AnnotatedString(state.fingerprint))
                        Toast.makeText(context, "设备指纹已复制到剪贴板", Toast.LENGTH_SHORT).show()
                    }
                },
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.End,
                ) {
                    ValueBadge(
                        text = displayFp,
                        mono = true,
                        accent = fingerprintVisible,
                        onClick = {
                            if (state.fingerprint.isNotEmpty()) {
                                clipboardManager.setText(AnnotatedString(state.fingerprint))
                                Toast.makeText(context, "设备指纹已复制到剪贴板", Toast.LENGTH_SHORT).show()
                            }
                        },
                        modifier = Modifier.weight(1f, fill = false),
                    )
                    Spacer(Modifier.width(4.dp))
                    IconButton(
                        onClick = { fingerprintVisible = !fingerprintVisible },
                        modifier = Modifier.size(34.dp),
                    ) {
                        Icon(
                            painter = painterResource(
                                if (fingerprintVisible) R.drawable.ic_visibility_off else R.drawable.ic_visibility
                            ),
                            contentDescription = if (fingerprintVisible) "隐藏安全指纹" else "显示安全指纹",
                            tint = if (fingerprintVisible) {
                                MaterialTheme.colorScheme.primary
                            } else {
                                MaterialTheme.colorScheme.onSurfaceVariant
                            },
                            modifier = Modifier.size(20.dp),
                        )
                    }
                }
            }

            SettingDivider()

            SettingRow(
                title = "传输引擎版本",
                subtitle = "LocalTransfer Rust P2P Core 核心底层引擎",
            ) {
                ValueBadge(
                    text = state.version.ifEmpty { "0.1.0" },
                    mono = true,
                )
            }
        }
    }
}

// ---------------- 通用组件与立体设计 ----------------

@Composable
private fun SectionHeader(title: String) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.padding(start = 4.dp, top = 6.dp, bottom = 2.dp),
    ) {
        Box(
            modifier = Modifier
                .width(3.5.dp)
                .height(14.dp)
                .background(
                    color = MaterialTheme.colorScheme.primary,
                    shape = RoundedCornerShape(2.dp),
                ),
        )
        Text(
            text = title,
            style = MaterialTheme.typography.titleSmall,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.primary,
            letterSpacing = 0.3.sp,
        )
    }
}

@Composable
private fun SettingCard(
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    Card(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(18.dp),
        elevation = CardDefaults.cardElevation(
            defaultElevation = 2.5.dp,
            pressedElevation = 4.dp,
        ),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surface,
        ),
        border = BorderStroke(
            1.dp,
            MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.22f),
        ),
    ) {
        Column(content = content)
    }
}

@Composable
private fun SettingDivider() {
    HorizontalDivider(
        modifier = Modifier.padding(horizontal = 16.dp),
        thickness = 0.6.dp,
        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.2f),
    )
}

/**
 * 优化后的设置行组件：
 * 1. 行内对齐：第一行容纳参数名（左）与具体的参数值/交互控件（右对齐）；
 * 2. 独立描述：第二行独立展示补充说明，字体更小更淡更轻；
 * 3. 换行自适应：参数值禁止省略截断，超宽时自适应折行完整展示。
 */
@Composable
private fun SettingRow(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    onClick: (() -> Unit)? = null,
    alignTop: Boolean = false,
    trailing: @Composable () -> Unit,
) {
    val rowModifier = if (onClick != null) {
        modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 13.dp)
    } else {
        modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 13.dp)
    }

    Column(modifier = rowModifier) {
        // 第一行：左侧参数名称，右侧参数值/控件，严格处于同一行
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = if (alignTop) Alignment.Top else Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier
                    .weight(1f, fill = false)
                    .then(if (alignTop) Modifier.padding(top = 4.dp) else Modifier),
            )
            Spacer(Modifier.width(12.dp))
            Box(
                modifier = Modifier.weight(2f, fill = false),
                contentAlignment = Alignment.CenterEnd,
            ) {
                trailing()
            }
        }

        // 第二行：参数描述单独另起一行
        if (!subtitle.isNullOrEmpty()) {
            Text(
                text = subtitle,
                style = MaterialTheme.typography.labelSmall,
                fontWeight = FontWeight.Normal,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.68f),
                lineHeight = 15.sp,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
    }
}

/**
 * 参数值徽标/芯片：
 * - 具备精致的圆角微立体边框与背景；
 * - 文本靠右对齐，softWrap 自动换行，绝不省略；
 * - 支持等宽字体与强调色模式。
 */
@Composable
private fun ValueBadge(
    text: String,
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    mono: Boolean = false,
    accent: Boolean = false,
) {
    Surface(
        onClick = onClick ?: {},
        enabled = onClick != null,
        shape = RoundedCornerShape(10.dp),
        color = if (accent) {
            MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.35f)
        } else {
            MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
        },
        border = BorderStroke(
            0.8.dp,
            if (accent) {
                MaterialTheme.colorScheme.primary.copy(alpha = 0.35f)
            } else {
                MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.3f)
            },
        ),
        modifier = modifier,
    ) {
        Text(
            text = text,
            style = MaterialTheme.typography.bodySmall,
            fontWeight = FontWeight.Medium,
            fontFamily = if (mono) FontFamily.Monospace else FontFamily.Default,
            color = if (accent) {
                MaterialTheme.colorScheme.primary
            } else {
                MaterialTheme.colorScheme.onSurface
            },
            textAlign = TextAlign.End,
            softWrap = true,
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
        )
    }
}

// ---------------- 弹窗组件 ----------------

@Composable
private fun EditNameDialog(
    current: String,
    onDismiss: () -> Unit,
    onConfirm: (String) -> Unit,
) {
    var name by remember { mutableStateOf(current) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("修改设备名称") },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text("设备名称") },
                placeholder = { Text("如：我的手机") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            Button(
                enabled = name.isNotBlank(),
                onClick = { onConfirm(name) },
            ) {
                Text("保存")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("取消")
            }
        },
    )
}

@Composable
private fun EditPortDialog(
    current: Int,
    onDismiss: () -> Unit,
    onConfirm: (Int) -> Unit,
) {
    var portText by remember { mutableStateOf(current.toString()) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("修改广播探测端口") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                OutlinedTextField(
                    value = portText,
                    onValueChange = { portText = it.filter(Char::isDigit).take(5) },
                    label = { Text("端口号（1-65535）") },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Text(
                    text = "修改后将在当前传输任务全部结束后生效。",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.75f),
                )
            }
        },
        confirmButton = {
            val valid = portText.toIntOrNull()?.let { it in 1..65535 } == true
            Button(
                enabled = valid,
                onClick = { onConfirm(portText.toInt()) },
            ) {
                Text("保存")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("取消")
            }
        },
    )
}

// ---------------- 工具常量与解析 ----------------

private val CHUNK_OPTIONS = listOf(
    256L * 1024 to "256KB",
    512L * 1024 to "512KB",
    1024L * 1024 to "1MB（推荐）",
    4L * 1024 * 1024 to "4MB",
)

private fun chunkLabel(size: Long): String =
    CHUNK_OPTIONS.firstOrNull { it.first == size }?.second ?: "${size / 1024}KB"

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
