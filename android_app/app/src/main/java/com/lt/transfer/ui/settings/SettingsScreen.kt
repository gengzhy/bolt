package com.lt.transfer.ui.settings

import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import android.app.Activity
import android.app.LocaleManager
import android.content.Context
import android.os.Build
import android.os.LocaleList
import java.util.Locale
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
import androidx.compose.ui.res.stringResource
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
        SectionHeader(stringResource(R.string.settings_section_device))
        SettingCard {
            val unnamed = stringResource(R.string.setting_device_name_unnamed)
            SettingRow(
                title = stringResource(R.string.setting_device_name_title),
                subtitle = stringResource(R.string.setting_device_name_desc),
                onClick = { showNameDialog = true },
            ) {
                ValueBadge(
                    text = "${state.config.deviceName.ifEmpty { unnamed }}  ✎",
                    accent = true,
                    onClick = { showNameDialog = true },
                )
            }

            SettingDivider()

            // 界面语言 / Language
            var langMenuOpen by remember { mutableStateOf(false) }
            val context = LocalContext.current
            val currentLangTag = getCurrentLocaleTag(context)
            val currentLangLabel = when {
                currentLangTag.startsWith("en", ignoreCase = true) -> stringResource(R.string.language_en_dropdown)
                currentLangTag.startsWith("zh", ignoreCase = true) -> stringResource(R.string.language_zh_dropdown)
                else -> stringResource(R.string.language_system_dropdown)
            }

            SettingRow(
                title = stringResource(R.string.setting_language_title),
                subtitle = stringResource(R.string.setting_language_desc),
                onClick = { langMenuOpen = true },
            ) {
                Box {
                    ValueBadge(
                        text = currentLangLabel,
                        accent = true,
                        onClick = { langMenuOpen = true },
                    )
                    DropdownMenu(
                        expanded = langMenuOpen,
                        onDismissRequest = { langMenuOpen = false },
                    ) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.language_system)) },
                            onClick = {
                                langMenuOpen = false
                                setAppLocale(context, "")
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.language_zh)) },
                            onClick = {
                                langMenuOpen = false
                                setAppLocale(context, "zh-CN")
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.language_en)) },
                            onClick = {
                                langMenuOpen = false
                                setAppLocale(context, "en")
                            },
                        )
                    }
                }
            }

            SettingDivider()

            val emptyPlaceholder = stringResource(R.string.common_empty_placeholder)
            val ipList = state.localIps.ifEmpty { listOf(emptyPlaceholder) }
            val ipText = ipList.joinToString("、")
            SettingRow(
                title = stringResource(R.string.setting_local_ip_title),
                subtitle = stringResource(R.string.setting_local_ip_desc),
                alignTop = ipList.size > 1,
            ) {
                ValueBadge(
                    text = ipText,
                    mono = true,
                )
            }

            SettingDivider()

            SettingRow(
                title = stringResource(R.string.setting_listen_port_actual_title),
                subtitle = stringResource(R.string.setting_listen_port_actual_desc),
            ) {
                ValueBadge(
                    text = if (state.localPort > 0) state.localPort.toString() else emptyPlaceholder,
                    mono = true,
                )
            }

            SettingDivider()

            SettingRow(
                title = stringResource(R.string.setting_stealth_mode_title),
                subtitle = stringResource(R.string.setting_stealth_mode_desc),
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
        SectionHeader(stringResource(R.string.settings_section_network))
        SettingCard {
            // 传输协议
            SettingRow(
                title = stringResource(R.string.setting_protocol_title),
                subtitle = stringResource(R.string.setting_protocol_desc),
                onClick = { protoMenuOpen = true },
            ) {
                Box {
                    ValueBadge(
                        text = if (state.config.preferQuic) stringResource(R.string.proto_quic_recommend_dropdown) else stringResource(R.string.proto_tcp_dropdown),
                        accent = state.config.preferQuic,
                        onClick = { protoMenuOpen = true },
                    )
                    DropdownMenu(
                        expanded = protoMenuOpen,
                        onDismissRequest = { protoMenuOpen = false },
                    ) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.proto_quic_recommend)) },
                            onClick = {
                                protoMenuOpen = false
                                if (!state.config.preferQuic) {
                                    LtEngine.setConfig(JSONObject().put("prefer_quic", true))
                                }
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.proto_tcp)) },
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
                title = stringResource(R.string.setting_concurrency_title),
                subtitle = stringResource(R.string.setting_concurrency_desc),
                onClick = { concurrencyMenuOpen = true },
            ) {
                Box {
                    val currentC = if (state.config.concurrency > 0) state.config.concurrency else 4
                    ValueBadge(
                        text = stringResource(R.string.concurrency_streams_format, currentC),
                        onClick = { concurrencyMenuOpen = true },
                    )
                    DropdownMenu(
                        expanded = concurrencyMenuOpen,
                        onDismissRequest = { concurrencyMenuOpen = false },
                    ) {
                        listOf(1, 2, 4, 8, 12, 16).forEach { num ->
                            DropdownMenuItem(
                                text = {
                                    Text(
                                        if (num == 4) stringResource(R.string.concurrency_stream_item_recommend_format, num)
                                        else stringResource(R.string.concurrency_stream_item_format, num)
                                    )
                                },
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
                title = stringResource(R.string.setting_chunk_size_title),
                subtitle = stringResource(R.string.setting_chunk_size_desc),
                onClick = { chunkMenuOpen = true },
            ) {
                Box {
                    ValueBadge(
                        text = stringResource(R.string.chunk_size_format, chunkLabel(state.config.chunkSize)),
                        onClick = { chunkMenuOpen = true },
                    )
                    DropdownMenu(
                        expanded = chunkMenuOpen,
                        onDismissRequest = { chunkMenuOpen = false },
                    ) {
                        CHUNK_OPTIONS.forEach { (size, resId) ->
                            DropdownMenuItem(
                                text = { Text(stringResource(resId)) },
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
                Toast.makeText(context, context.getString(R.string.setting_save_dir_updated_toast, finalDir), Toast.LENGTH_SHORT).show()
            } else {
                Toast.makeText(context, context.getString(R.string.setting_save_dir_invalid_toast), Toast.LENGTH_LONG).show()
            }
        }
    }

    var collisionMenuOpen by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SectionHeader(stringResource(R.string.settings_section_receive))
        SettingCard {
            // 接收保存目录：参数名称与参数值同一行，参数值超宽自动折行不截断；补充说明另起一行
            val safeWrappedPath = currentSaveDir.replace("/", "/\u200B")
            val btnChange = stringResource(R.string.setting_save_dir_btn_change)
            SettingRow(
                title = stringResource(R.string.setting_save_dir_title),
                subtitle = stringResource(R.string.setting_save_dir_desc),
                alignTop = true,
                onClick = { folderPicker.launch(null) },
            ) {
                ValueBadge(
                    text = "$safeWrappedPath  $btnChange",
                    mono = true,
                    accent = true,
                    onClick = { folderPicker.launch(null) },
                )
            }

            SettingDivider()

            // 同名文件冲突策略
            SettingRow(
                title = stringResource(R.string.setting_collision_title),
                subtitle = stringResource(R.string.setting_collision_desc),
                onClick = { collisionMenuOpen = true },
            ) {
                Box {
                    ValueBadge(
                        text = if (state.config.collision == "overwrite") stringResource(R.string.collision_overwrite_dropdown) else stringResource(R.string.collision_rename_dropdown),
                        onClick = { collisionMenuOpen = true },
                    )
                    DropdownMenu(
                        expanded = collisionMenuOpen,
                        onDismissRequest = { collisionMenuOpen = false },
                    ) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.collision_rename)) },
                            onClick = {
                                collisionMenuOpen = false
                                if (state.config.collision != "rename") {
                                    LtEngine.setConfig(JSONObject().put("collision", "rename"))
                                }
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.collision_overwrite)) },
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
                title = stringResource(R.string.setting_auto_accept_title),
                subtitle = stringResource(R.string.setting_auto_accept_desc),
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
        SectionHeader(stringResource(R.string.settings_section_discovery))
        SettingCard {
            SettingRow(
                title = stringResource(R.string.setting_mdns_title),
                subtitle = stringResource(R.string.setting_mdns_desc),
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
                title = stringResource(R.string.setting_probe_port_title),
                subtitle = stringResource(R.string.setting_probe_port_desc),
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
        SectionHeader(stringResource(R.string.settings_section_maintenance))
        SettingCard {
            SettingRow(
                title = stringResource(R.string.setting_records_title),
                subtitle = stringResource(R.string.setting_records_desc),
            ) {
                OutlinedButton(
                    shape = RoundedCornerShape(10.dp),
                    onClick = {
                        LtEngine.clearRecords()
                        Toast.makeText(context, context.getString(R.string.setting_records_cleared_toast), Toast.LENGTH_SHORT).show()
                    },
                ) {
                    Text(stringResource(R.string.setting_records_btn_clear))
                }
            }

            SettingDivider()

            SettingRow(
                title = stringResource(R.string.setting_cache_title),
                subtitle = stringResource(R.string.setting_cache_desc),
            ) {
                OutlinedButton(
                    shape = RoundedCornerShape(10.dp),
                    onClick = {
                        LtEngine.clearTempCache()
                        Toast.makeText(context, context.getString(R.string.setting_cache_cleared_toast), Toast.LENGTH_SHORT).show()
                    },
                ) {
                    Text(stringResource(R.string.setting_cache_btn_clear))
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
        SectionHeader(stringResource(R.string.settings_section_about))
        SettingCard {
            val fp = state.fingerprint.ifEmpty { "—" }
            val maskedFp = "••••••••••••"
            val displayFp = if (fingerprintVisible) {
                fp.replace(":", ":\u200B")
            } else {
                maskedFp
            }

            SettingRow(
                title = stringResource(R.string.setting_fingerprint_title),
                subtitle = stringResource(R.string.setting_fingerprint_desc),
                alignTop = fingerprintVisible,
                onClick = {
                    if (state.fingerprint.isNotEmpty()) {
                        clipboardManager.setText(AnnotatedString(state.fingerprint))
                        Toast.makeText(context, context.getString(R.string.setting_fingerprint_copied_toast), Toast.LENGTH_SHORT).show()
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
                                Toast.makeText(context, context.getString(R.string.setting_fingerprint_copied_toast), Toast.LENGTH_SHORT).show()
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
                            contentDescription = if (fingerprintVisible) {
                                stringResource(R.string.setting_fingerprint_hide)
                            } else {
                                stringResource(R.string.setting_fingerprint_show)
                            },
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
                title = stringResource(R.string.setting_engine_ver_title),
                subtitle = stringResource(R.string.setting_engine_ver_desc),
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
        title = { Text(stringResource(R.string.dialog_edit_name_title)) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text(stringResource(R.string.setting_device_name_title)) },
                placeholder = { Text(stringResource(R.string.dialog_edit_name_placeholder)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            Button(
                enabled = name.isNotBlank(),
                onClick = { onConfirm(name) },
            ) {
                Text(stringResource(R.string.common_save))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.common_cancel))
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
        title = { Text(stringResource(R.string.dialog_edit_port_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                OutlinedTextField(
                    value = portText,
                    onValueChange = { portText = it.filter(Char::isDigit).take(5) },
                    label = { Text(stringResource(R.string.dialog_edit_port_label)) },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Text(
                    text = stringResource(R.string.dialog_edit_port_hint),
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
                Text(stringResource(R.string.common_save))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.common_cancel))
            }
        },
    )
}

// ---------------- 工具常量与解析 ----------------

private val CHUNK_OPTIONS = listOf(
    256L * 1024 to R.string.chunk_size_256kb,
    512L * 1024 to R.string.chunk_size_512kb,
    1024L * 1024 to R.string.chunk_size_1mb,
    4L * 1024 * 1024 to R.string.chunk_size_4mb,
)

@Composable
private fun chunkLabel(size: Long): String {
    val resId = CHUNK_OPTIONS.firstOrNull { it.first == size }?.second
    return if (resId != null) stringResource(resId) else "${size / 1024}KB"
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

private fun getCurrentLocaleTag(context: Context): String {
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        val localeManager = context.getSystemService(LocaleManager::class.java)
        localeManager?.applicationLocales?.toLanguageTags() ?: ""
    } else {
        context.resources.configuration.locales[0]?.toLanguageTag() ?: ""
    }
}

private fun setAppLocale(context: Context, tag: String) {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        val localeManager = context.getSystemService(LocaleManager::class.java)
        localeManager?.applicationLocales = if (tag.isEmpty()) {
            LocaleList.getEmptyLocaleList()
        } else {
            LocaleList.forLanguageTags(tag)
        }
    } else {
        val locale = if (tag.isEmpty()) Locale.getDefault() else Locale.forLanguageTag(tag)
        Locale.setDefault(locale)
        val config = context.resources.configuration
        config.setLocale(locale)
        @Suppress("DEPRECATION")
        context.resources.updateConfiguration(config, context.resources.displayMetrics)
        (context as? Activity)?.recreate()
    }
}

