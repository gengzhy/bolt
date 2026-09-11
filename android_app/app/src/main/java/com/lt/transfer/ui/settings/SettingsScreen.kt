package com.lt.transfer.ui.settings

import android.app.Activity
import android.app.LocaleManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.LocaleList
import android.provider.DocumentsContract
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
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
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
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
import androidx.compose.ui.unit.dp
import com.lt.transfer.R
import com.lt.transfer.engine.LtEngine
import com.lt.transfer.model.UiState
import com.lt.transfer.ui.components.card.LtCard
import com.lt.transfer.ui.components.dialogs.LtValueEditDialog
import com.lt.transfer.ui.components.form.LtDropdownOption
import com.lt.transfer.ui.components.form.LtDropdownPicker
import com.lt.transfer.ui.components.form.LtSettingDivider
import com.lt.transfer.ui.components.form.LtSettingRow
import com.lt.transfer.ui.components.form.LtSettingSectionHeader
import com.lt.transfer.ui.components.form.LtSwitchRow
import com.lt.transfer.ui.components.form.LtValueBadge
import com.lt.transfer.ui.components.motion.AnimatedTextSwitcher
import org.json.JSONObject
import java.io.File
import java.util.Locale

/**
 * 设置页（基于统一设计系统组件库全面重构版）：
 * 彻底剔除样板代码，全量采用 components.form / card / dialogs / motion 模块。
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

        // 底部品牌展示区：logo、APP名称、版本、版权分别单独成行展示
        val currentYear = remember { java.util.Calendar.getInstance().get(java.util.Calendar.YEAR) }
        val author = stringResource(R.string.app_author)
        val appName = stringResource(R.string.app_name)
        val appVersion = stringResource(R.string.app_version_fmt, state.version.ifEmpty { "0.1.0" })
        val copyright = stringResource(R.string.app_copyright, currentYear, author)

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 10.dp, bottom = 12.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            // 1. Logo
            androidx.compose.foundation.Image(
                painter = painterResource(R.drawable.ic_logo_brand),
                contentDescription = "LocalTransfer Logo",
                modifier = Modifier
                    .padding(bottom = 4.dp)
                    .size(width = 54.dp, height = 38.dp),
                contentScale = androidx.compose.ui.layout.ContentScale.Fit,
            )
            // 2. APP名称（粗体）
            Text(
                text = appName,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = androidx.compose.ui.text.font.FontWeight.Bold,
                color = MaterialTheme.colorScheme.onSurface,
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
            // 3. 版本
            Text(
                text = appVersion,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
            // 4. 版权（@当前年份 + 作者）
            Text(
                text = copyright,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
        }

        Spacer(Modifier.height(16.dp))
    }
}

// ---------------- 1. 本机设备模块 ----------------

@Composable
private fun DeviceModule(state: UiState) {
    var showNameDialog by remember { mutableStateOf(false) }
    val context = LocalContext.current

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        LtSettingSectionHeader(stringResource(R.string.settings_section_device))
        LtCard {
            val unnamed = stringResource(R.string.setting_device_name_unnamed)
            LtSettingRow(
                title = stringResource(R.string.setting_device_name_title),
                subtitle = stringResource(R.string.setting_device_name_desc),
                onClick = { showNameDialog = true },
            ) {
                LtValueBadge(
                    text = "${state.config.deviceName.ifEmpty { unnamed }}  ✎",
                    accent = true,
                    onClick = { showNameDialog = true },
                )
            }

            LtSettingDivider()

            // 界面语言 / Language
            val currentLangTag = getCurrentLocaleTag(context)
            val langOptions = listOf(
                LtDropdownOption("", stringResource(R.string.language_system_dropdown)),
                LtDropdownOption("zh-CN", stringResource(R.string.language_zh_dropdown)),
                LtDropdownOption("en", stringResource(R.string.language_en_dropdown)),
            )
            val selectedLang = when {
                currentLangTag.startsWith("en", ignoreCase = true) -> "en"
                currentLangTag.startsWith("zh", ignoreCase = true) -> "zh-CN"
                else -> ""
            }
            LtDropdownPicker(
                title = stringResource(R.string.setting_language_title),
                subtitle = stringResource(R.string.setting_language_desc),
                options = langOptions,
                selectedKey = selectedLang,
                onSelect = { tag -> setAppLocale(context, tag) },
                isAccent = true,
            )

            LtSettingDivider()

            val emptyPlaceholder = stringResource(R.string.common_empty_placeholder)
            val ipList = state.localIps.ifEmpty { listOf(emptyPlaceholder) }
            val ipText = ipList.joinToString("、")
            LtSettingRow(
                title = stringResource(R.string.setting_local_ip_title),
                subtitle = stringResource(R.string.setting_local_ip_desc),
                alignTop = ipList.size > 1,
            ) {
                LtValueBadge(
                    text = ipText,
                    mono = true,
                )
            }

            LtSettingDivider()

            LtSettingRow(
                title = stringResource(R.string.setting_listen_port_actual_title),
                subtitle = stringResource(R.string.setting_listen_port_actual_desc),
            ) {
                LtValueBadge(
                    text = if (state.localPort > 0) state.localPort.toString() else emptyPlaceholder,
                    mono = true,
                )
            }

            LtSettingDivider()

            LtSwitchRow(
                title = stringResource(R.string.setting_stealth_mode_title),
                subtitle = stringResource(R.string.setting_stealth_mode_desc),
                checked = state.config.stealthMode,
                onCheckedChange = { LtEngine.setConfig(JSONObject().put("stealth_mode", it)) },
            )
        }
    }

    if (showNameDialog) {
        LtValueEditDialog(
            title = stringResource(R.string.dialog_edit_name_title),
            initialValue = state.config.deviceName,
            label = stringResource(R.string.setting_device_name_title),
            confirmText = stringResource(R.string.common_save),
            dismissText = stringResource(R.string.common_cancel),
            onConfirm = { newName ->
                showNameDialog = false
                if (newName.isNotBlank() && newName != state.config.deviceName) {
                    LtEngine.setConfig(JSONObject().put("device_name", newName.trim()))
                }
            },
            onDismiss = { showNameDialog = false },
        )
    }
}

// ---------------- 2. 网络传输模块 ----------------

@Composable
private fun NetworkModule(state: UiState) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        LtSettingSectionHeader(stringResource(R.string.settings_section_network))
        LtCard {
            // 传输协议
            val protoOptions = listOf(
                LtDropdownOption(true, stringResource(R.string.proto_quic_recommend_dropdown)),
                LtDropdownOption(false, stringResource(R.string.proto_tcp_dropdown)),
            )
            LtDropdownPicker(
                title = stringResource(R.string.setting_protocol_title),
                subtitle = stringResource(R.string.setting_protocol_desc),
                options = protoOptions,
                selectedKey = state.config.preferQuic,
                onSelect = { preferQuic ->
                    LtEngine.setConfig(JSONObject().put("prefer_quic", preferQuic))
                },
                isAccent = state.config.preferQuic,
            )

            LtSettingDivider()

            // 并发流数量
            val currentC = if (state.config.concurrency > 0) state.config.concurrency else 4
            val concurrencyOptions = listOf(1, 2, 4, 8, 12, 16).map { num ->
                val label = if (num == 4) {
                    stringResource(R.string.concurrency_stream_item_recommend_format, num)
                } else {
                    stringResource(R.string.concurrency_stream_item_format, num)
                }
                LtDropdownOption(num, label)
            }
            LtDropdownPicker(
                title = stringResource(R.string.setting_concurrency_title),
                subtitle = stringResource(R.string.setting_concurrency_desc),
                options = concurrencyOptions,
                selectedKey = currentC,
                onSelect = { num ->
                    LtEngine.setConfig(JSONObject().put("concurrency", num))
                },
            )

            LtSettingDivider()

            // 分片大小
            val chunkOptions = CHUNK_OPTIONS.map { (size, resId) ->
                LtDropdownOption(size, stringResource(resId))
            }
            LtDropdownPicker(
                title = stringResource(R.string.setting_chunk_size_title),
                subtitle = stringResource(R.string.setting_chunk_size_desc),
                options = chunkOptions,
                selectedKey = state.config.chunkSize,
                onSelect = { size ->
                    LtEngine.setConfig(JSONObject().put("chunk_size", size))
                },
            )
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

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        LtSettingSectionHeader(stringResource(R.string.settings_section_receive))
        LtCard {
            val safeWrappedPath = currentSaveDir.replace("/", "/\u200B")
            val btnChange = stringResource(R.string.setting_save_dir_btn_change)
            LtSettingRow(
                title = stringResource(R.string.setting_save_dir_title),
                subtitle = stringResource(R.string.setting_save_dir_desc),
                alignTop = true,
                onClick = { folderPicker.launch(null) },
            ) {
                LtValueBadge(
                    text = "$safeWrappedPath  $btnChange",
                    mono = true,
                    accent = true,
                    onClick = { folderPicker.launch(null) },
                )
            }

            LtSettingDivider()

            // 同名文件冲突策略
            val collisionOptions = listOf(
                LtDropdownOption("rename", stringResource(R.string.collision_rename)),
                LtDropdownOption("overwrite", stringResource(R.string.collision_overwrite)),
            )
            LtDropdownPicker(
                title = stringResource(R.string.setting_collision_title),
                subtitle = stringResource(R.string.setting_collision_desc),
                options = collisionOptions,
                selectedKey = if (state.config.collision == "overwrite") "overwrite" else "rename",
                onSelect = { policy ->
                    LtEngine.setConfig(JSONObject().put("collision", policy))
                },
            )

            LtSettingDivider()

            // 自动接收已信任设备
            LtSwitchRow(
                title = stringResource(R.string.setting_auto_accept_title),
                subtitle = stringResource(R.string.setting_auto_accept_desc),
                checked = state.config.autoAcceptTrusted,
                onCheckedChange = { LtEngine.setConfig(JSONObject().put("auto_accept_trusted", it)) },
            )
        }
    }
}

// ---------------- 4. 设备发现模块 ----------------

@Composable
private fun DiscoveryModule(state: UiState) {
    var showPortDialog by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        LtSettingSectionHeader(stringResource(R.string.settings_section_discovery))
        LtCard {
            LtSwitchRow(
                title = stringResource(R.string.setting_mdns_title),
                subtitle = stringResource(R.string.setting_mdns_desc),
                checked = state.config.useMdns,
                onCheckedChange = { LtEngine.setConfig(JSONObject().put("use_mdns", it)) },
            )

            LtSettingDivider()

            LtSettingRow(
                title = stringResource(R.string.setting_probe_port_title),
                subtitle = stringResource(R.string.setting_probe_port_desc),
                onClick = { showPortDialog = true },
            ) {
                LtValueBadge(
                    text = "${if (state.config.listenPort > 0) state.config.listenPort else 8899}  ✎",
                    mono = true,
                    accent = true,
                    onClick = { showPortDialog = true },
                )
            }
        }
    }

    if (showPortDialog) {
        val currentPort = if (state.config.listenPort > 0) state.config.listenPort else 8899
        LtValueEditDialog(
            title = stringResource(R.string.dialog_edit_port_title),
            initialValue = currentPort.toString(),
            label = stringResource(R.string.dialog_edit_port_label),
            isNumber = true,
            confirmText = stringResource(R.string.common_save),
            dismissText = stringResource(R.string.common_cancel),
            validator = { it.toIntOrNull()?.let { p -> p in 1..65535 } == true },
            onConfirm = { portStr ->
                showPortDialog = false
                portStr.toIntOrNull()?.let { newPort ->
                    if (newPort in 1..65535 && newPort != state.config.listenPort) {
                        LtEngine.setConfig(JSONObject().put("listen_port", newPort))
                    }
                }
            },
            onDismiss = { showPortDialog = false },
        )
    }
}

// ---------------- 5. 存储与维护模块 ----------------

@Composable
private fun MaintenanceModule(state: UiState) {
    val context = LocalContext.current

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        LtSettingSectionHeader(stringResource(R.string.settings_section_maintenance))
        LtCard {
            LtSettingRow(
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

            LtSettingDivider()

            LtSettingRow(
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
        LtSettingSectionHeader(stringResource(R.string.settings_section_about))
        LtCard {
            val fp = state.fingerprint.ifEmpty { "—" }
            val maskedFp = "••••••••••••"
            val displayFp = if (fingerprintVisible) {
                fp.replace(":", ":\u200B")
            } else {
                maskedFp
            }

            LtSettingRow(
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
                    LtValueBadge(
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

            LtSettingDivider()

            LtSettingRow(
                title = stringResource(R.string.setting_engine_ver_title),
                subtitle = stringResource(R.string.setting_engine_ver_desc),
            ) {
                LtValueBadge(
                    text = state.version.ifEmpty { "0.1.0" },
                    mono = true,
                )
            }
        }
    }
}

// ---------------- 工具常量与解析 ----------------

private val CHUNK_OPTIONS = listOf(
    256L * 1024 to R.string.chunk_size_256kb,
    512L * 1024 to R.string.chunk_size_512kb,
    1024L * 1024 to R.string.chunk_size_1mb,
    4L * 1024 * 1024 to R.string.chunk_size_4mb,
)

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
