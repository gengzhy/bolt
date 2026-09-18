package xin.cosmos.bolt.ui.settings

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
import androidx.compose.foundation.clickable
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
import androidx.compose.ui.unit.sp
import xin.cosmos.bolt.R
import xin.cosmos.bolt.data.TransferLogRepository
import xin.cosmos.bolt.engine.BtEngine
import xin.cosmos.bolt.model.UiState
import xin.cosmos.bolt.ui.components.card.BtCard
import xin.cosmos.bolt.ui.components.dialogs.BtPrivacyPolicyDialog
import xin.cosmos.bolt.ui.components.dialogs.BtValueEditDialog
import xin.cosmos.bolt.ui.components.form.BtDropdownOption
import xin.cosmos.bolt.ui.components.form.BtDropdownPicker
import xin.cosmos.bolt.ui.components.form.BtSettingDivider
import xin.cosmos.bolt.ui.components.form.BtSettingRow
import xin.cosmos.bolt.ui.components.form.BtSettingSectionHeader
import xin.cosmos.bolt.ui.components.form.BtSwitchRow
import xin.cosmos.bolt.ui.components.form.BtValueBadge
import xin.cosmos.bolt.ui.components.motion.AnimatedTextSwitcher
import org.json.JSONObject
import java.io.File
import java.util.Locale

/**
 * 设置页（基于统一设计系统组件库全面重构版）：
 * 彻底剔除样板代码，全量采用 components.form / card / dialogs / motion 模块。
 */
@Composable
fun SettingsScreen(modifier: Modifier) {
    val state by BtEngine.uiState.collectAsState()
    var showPrivacyPolicyDialog by remember { mutableStateOf(false) }

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
        AboutModule(
            state = state,
            onShowPrivacyPolicy = { showPrivacyPolicyDialog = true },
        )

        // 底部品牌展示区：logo、APP名称、版本、隐私政策、版权分别单独成行展示
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
            // 1. Logo（放大2倍）
            androidx.compose.foundation.Image(
                painter = painterResource(R.drawable.ic_logo_brand),
                contentDescription = "Bolt Logo",
                modifier = Modifier
                    .padding(bottom = 8.dp)
                    .size(width = 108.dp, height = 76.dp),
                contentScale = androidx.compose.ui.layout.ContentScale.Fit,
            )
            // 2. APP名称（大字号粗体，协调匹配108dp大Logo）
            Text(
                text = appName,
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = androidx.compose.ui.text.font.FontWeight.Bold,
                letterSpacing = 0.5.sp,
                color = MaterialTheme.colorScheme.onSurface,
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
            // 3. 版本
            Text(
                text = appVersion,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
            // 4. 隐私政策入口
            Text(
                text = "《" + stringResource(R.string.setting_privacy_policy_title) + "》",
                style = MaterialTheme.typography.bodySmall.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Medium),
                color = MaterialTheme.colorScheme.primary,
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                modifier = Modifier
                    .padding(vertical = 2.dp)
                    .clickable { showPrivacyPolicyDialog = true },
            )
            // 5. 版权（@当前年份 + 作者）
            Text(
                text = copyright,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
        }

        Spacer(Modifier.height(32.dp))
    }

    if (showPrivacyPolicyDialog) {
        BtPrivacyPolicyDialog(
            onDismiss = { showPrivacyPolicyDialog = false },
        )
    }
}

// ---------------- 1. 本机设备模块 ----------------

@Composable
private fun DeviceModule(state: UiState) {
    var showNameDialog by remember { mutableStateOf(false) }
    val context = LocalContext.current

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        BtSettingSectionHeader(stringResource(R.string.settings_section_device))
        BtCard {
            val unnamed = stringResource(R.string.setting_device_name_unnamed)
            BtSettingRow(
                title = stringResource(R.string.setting_device_name_title),
                subtitle = stringResource(R.string.setting_device_name_desc),
                onClick = { showNameDialog = true },
            ) {
                BtValueBadge(
                    text = "${state.config.deviceName.ifEmpty { unnamed }}  ✎",
                    accent = true,
                    onClick = { showNameDialog = true },
                )
            }

            BtSettingDivider()

            // 界面语言 / Language
            val currentLangTag = getCurrentLocaleTag(context)
            val langOptions = listOf(
                BtDropdownOption("", stringResource(R.string.language_system_dropdown)),
                BtDropdownOption("zh-CN", stringResource(R.string.language_zh_dropdown)),
                BtDropdownOption("en", stringResource(R.string.language_en_dropdown)),
            )
            val selectedLang = when {
                currentLangTag.startsWith("en", ignoreCase = true) -> "en"
                currentLangTag.startsWith("zh", ignoreCase = true) -> "zh-CN"
                else -> ""
            }
            BtDropdownPicker(
                title = stringResource(R.string.setting_language_title),
                subtitle = stringResource(R.string.setting_language_desc),
                options = langOptions,
                selectedKey = selectedLang,
                onSelect = { tag -> setAppLocale(context, tag) },
                isAccent = true,
            )

            BtSettingDivider()

            val emptyPlaceholder = stringResource(R.string.common_empty_placeholder)
            val ipList = state.localIps.ifEmpty { listOf(emptyPlaceholder) }
            val ipText = ipList.joinToString("、")
            BtSettingRow(
                title = stringResource(R.string.setting_local_ip_title),
                subtitle = stringResource(R.string.setting_local_ip_desc),
                alignTop = ipList.size > 1,
            ) {
                BtValueBadge(
                    text = ipText,
                    mono = true,
                )
            }

            BtSettingDivider()

            BtSettingRow(
                title = stringResource(R.string.setting_listen_port_actual_title),
                subtitle = stringResource(R.string.setting_listen_port_actual_desc),
            ) {
                BtValueBadge(
                    text = if (state.localPort > 0) state.localPort.toString() else emptyPlaceholder,
                    mono = true,
                )
            }

            BtSettingDivider()

            BtSwitchRow(
                title = stringResource(R.string.setting_stealth_mode_title),
                subtitle = stringResource(R.string.setting_stealth_mode_desc),
                checked = state.config.stealthMode,
                onCheckedChange = { BtEngine.setConfig(JSONObject().put("stealth_mode", it)) },
            )
        }
    }

    if (showNameDialog) {
        BtValueEditDialog(
            title = stringResource(R.string.dialog_edit_name_title),
            initialValue = state.config.deviceName,
            label = stringResource(R.string.setting_device_name_title),
            confirmText = stringResource(R.string.common_save),
            dismissText = stringResource(R.string.common_cancel),
            onConfirm = { newName ->
                showNameDialog = false
                if (newName.isNotBlank() && newName != state.config.deviceName) {
                    BtEngine.setConfig(JSONObject().put("device_name", newName.trim()))
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
        BtSettingSectionHeader(stringResource(R.string.settings_section_network))
        BtCard {
            // 传输协议
            val protoOptions = listOf(
                BtDropdownOption(true, stringResource(R.string.proto_quic_recommend_dropdown)),
                BtDropdownOption(false, stringResource(R.string.proto_tcp_dropdown)),
            )
            BtDropdownPicker(
                title = stringResource(R.string.setting_protocol_title),
                subtitle = stringResource(R.string.setting_protocol_desc),
                options = protoOptions,
                selectedKey = state.config.preferQuic,
                onSelect = { preferQuic ->
                    BtEngine.setConfig(JSONObject().put("prefer_quic", preferQuic))
                },
                isAccent = state.config.preferQuic,
            )

            BtSettingDivider()

            // 并发流数量
            val currentC = if (state.config.concurrency > 0) state.config.concurrency else 4
            val concurrencyOptions = listOf(1, 2, 4, 8, 12, 16).map { num ->
                val label = if (num == 4) {
                    stringResource(R.string.concurrency_stream_item_recommend_format, num)
                } else {
                    stringResource(R.string.concurrency_stream_item_format, num)
                }
                BtDropdownOption(num, label)
            }
            BtDropdownPicker(
                title = stringResource(R.string.setting_concurrency_title),
                subtitle = stringResource(R.string.setting_concurrency_desc),
                options = concurrencyOptions,
                selectedKey = currentC,
                onSelect = { num ->
                    BtEngine.setConfig(JSONObject().put("concurrency", num))
                },
            )

            BtSettingDivider()

            // 分片大小
            val chunkOptions = CHUNK_OPTIONS.map { (size, resId) ->
                BtDropdownOption(size, stringResource(resId))
            }
            BtDropdownPicker(
                title = stringResource(R.string.setting_chunk_size_title),
                subtitle = stringResource(R.string.setting_chunk_size_desc),
                options = chunkOptions,
                selectedKey = state.config.chunkSize,
                onSelect = { size ->
                    BtEngine.setConfig(JSONObject().put("chunk_size", size))
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
        File(Environment.getExternalStorageDirectory(), "Download/Bolt").absolutePath
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
                val finalDir = if (cleanParent.endsWith("/Bolt", ignoreCase = true) ||
                    cleanParent.endsWith("\\Bolt", ignoreCase = true)
                ) {
                    cleanParent
                } else {
                    "$cleanParent/Bolt"
                }

                val dir = File(finalDir)
                if (!dir.exists()) {
                    dir.mkdirs()
                }
                BtEngine.setConfig(JSONObject().put("save_dir", finalDir))
                Toast.makeText(context, context.getString(R.string.setting_save_dir_updated_toast, finalDir), Toast.LENGTH_SHORT).show()
            } else {
                Toast.makeText(context, context.getString(R.string.setting_save_dir_invalid_toast), Toast.LENGTH_LONG).show()
            }
        }
    }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        BtSettingSectionHeader(stringResource(R.string.settings_section_receive))
        BtCard {
            val safeWrappedPath = currentSaveDir.replace("/", "/\u200B")
            val btnChange = stringResource(R.string.setting_save_dir_btn_change)
            BtSettingRow(
                title = stringResource(R.string.setting_save_dir_title),
                subtitle = stringResource(R.string.setting_save_dir_desc),
                alignTop = true,
                onClick = { folderPicker.launch(null) },
            ) {
                BtValueBadge(
                    text = "$safeWrappedPath  $btnChange",
                    mono = true,
                    accent = true,
                    onClick = { folderPicker.launch(null) },
                )
            }

            BtSettingDivider()

            // 同名文件冲突策略
            val collisionOptions = listOf(
                BtDropdownOption("rename", stringResource(R.string.collision_rename)),
                BtDropdownOption("overwrite", stringResource(R.string.collision_overwrite)),
            )
            BtDropdownPicker(
                title = stringResource(R.string.setting_collision_title),
                subtitle = stringResource(R.string.setting_collision_desc),
                options = collisionOptions,
                selectedKey = if (state.config.collision == "overwrite") "overwrite" else "rename",
                onSelect = { policy ->
                    BtEngine.setConfig(JSONObject().put("collision", policy))
                },
            )

            BtSettingDivider()

            // 自动接收已信任设备
            BtSwitchRow(
                title = stringResource(R.string.setting_auto_accept_title),
                subtitle = stringResource(R.string.setting_auto_accept_desc),
                checked = state.config.autoAcceptTrusted,
                onCheckedChange = { BtEngine.setConfig(JSONObject().put("auto_accept_trusted", it)) },
            )
        }
    }
}

// ---------------- 4. 设备发现模块 ----------------

@Composable
private fun DiscoveryModule(state: UiState) {
    var showPortDialog by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        BtSettingSectionHeader(stringResource(R.string.settings_section_discovery))
        BtCard {
            BtSwitchRow(
                title = stringResource(R.string.setting_mdns_title),
                subtitle = stringResource(R.string.setting_mdns_desc),
                checked = state.config.useMdns,
                onCheckedChange = { BtEngine.setConfig(JSONObject().put("use_mdns", it)) },
            )

            BtSettingDivider()

            BtSettingRow(
                title = stringResource(R.string.setting_probe_port_title),
                subtitle = stringResource(R.string.setting_probe_port_desc),
                onClick = { showPortDialog = true },
            ) {
                BtValueBadge(
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
        BtValueEditDialog(
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
                        BtEngine.setConfig(JSONObject().put("listen_port", newPort))
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
    val maxLogsCount by TransferLogRepository.maxLogsCount.collectAsState()
    var showMaxLogsDialog by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        BtSettingSectionHeader(stringResource(R.string.settings_section_maintenance))
        BtCard {
            BtSettingRow(
                title = stringResource(R.string.setting_max_logs_title),
                subtitle = stringResource(R.string.setting_max_logs_desc),
                onClick = { showMaxLogsDialog = true },
            ) {
                BtValueBadge(
                    text = stringResource(R.string.setting_max_logs_unit, maxLogsCount) + "  ✎",
                    mono = true,
                    accent = true,
                    onClick = { showMaxLogsDialog = true },
                )
            }

            BtSettingDivider()

            BtSettingRow(
                title = stringResource(R.string.setting_records_title),
                subtitle = stringResource(R.string.setting_records_desc),
            ) {
                OutlinedButton(
                    shape = RoundedCornerShape(10.dp),
                    onClick = {
                        BtEngine.clearRecords()
                        Toast.makeText(context, context.getString(R.string.setting_records_cleared_toast), Toast.LENGTH_SHORT).show()
                    },
                ) {
                    Text(stringResource(R.string.setting_records_btn_clear))
                }
            }

            BtSettingDivider()

            BtSettingRow(
                title = stringResource(R.string.setting_cache_title),
                subtitle = stringResource(R.string.setting_cache_desc),
            ) {
                OutlinedButton(
                    shape = RoundedCornerShape(10.dp),
                    onClick = {
                        BtEngine.clearTempCache()
                        Toast.makeText(context, context.getString(R.string.setting_cache_cleared_toast), Toast.LENGTH_SHORT).show()
                    },
                ) {
                    Text(stringResource(R.string.setting_cache_btn_clear))
                }
            }
        }
    }

    if (showMaxLogsDialog) {
        BtValueEditDialog(
            title = stringResource(R.string.dialog_edit_max_logs_title),
            initialValue = maxLogsCount.toString(),
            label = stringResource(R.string.dialog_edit_max_logs_label),
            isNumber = true,
            confirmText = stringResource(R.string.common_save),
            dismissText = stringResource(R.string.common_cancel),
            validator = { it.toIntOrNull()?.let { count -> count in 1..1000000 } == true },
            onConfirm = { countStr ->
                showMaxLogsDialog = false
                countStr.toIntOrNull()?.let { newCount ->
                    if (newCount in 1..1000000 && newCount != maxLogsCount) {
                        TransferLogRepository.setMaxLogsCount(context, newCount)
                        Toast.makeText(context, context.getString(R.string.setting_max_logs_saved_toast), Toast.LENGTH_SHORT).show()
                    }
                }
            },
            onDismiss = { showMaxLogsDialog = false },
        )
    }
}

// ---------------- 6. 关于本机模块 ----------------

@Composable
private fun AboutModule(
    state: UiState,
    onShowPrivacyPolicy: () -> Unit,
) {
    val context = LocalContext.current
    val clipboardManager = LocalClipboardManager.current
    var fingerprintVisible by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        BtSettingSectionHeader(stringResource(R.string.settings_section_about))
        BtCard {
            val fp = state.fingerprint.ifEmpty { "—" }
            val maskedFp = "••••••••••••"
            val displayFp = if (fingerprintVisible) {
                fp.replace(":", ":\u200B")
            } else {
                maskedFp
            }

            BtSettingRow(
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
                    BtValueBadge(
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

            BtSettingDivider()

            BtSettingRow(
                title = stringResource(R.string.setting_engine_ver_title),
                subtitle = stringResource(R.string.setting_engine_ver_desc),
            ) {
                BtValueBadge(
                    text = state.version.ifEmpty { "0.1.0" },
                    mono = true,
                )
            }

            BtSettingDivider()

            BtSettingRow(
                title = stringResource(R.string.setting_privacy_policy_title),
                subtitle = stringResource(R.string.setting_privacy_policy_desc),
                onClick = onShowPrivacyPolicy,
            ) {
                Icon(
                    painter = painterResource(R.drawable.ic_chevron_right),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(20.dp),
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
