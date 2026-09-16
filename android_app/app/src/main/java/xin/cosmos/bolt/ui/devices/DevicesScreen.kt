package xin.cosmos.bolt.ui.devices

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContract
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import xin.cosmos.bolt.R
import xin.cosmos.bolt.ui.components.card.BtCard
import xin.cosmos.bolt.ui.components.feedback.EmptyStateView
import xin.cosmos.bolt.ui.components.motion.AnimatedCollapse
import androidx.compose.material3.ButtonDefaults
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xin.cosmos.bolt.engine.BtEngine
import xin.cosmos.bolt.engine.SendStager
import xin.cosmos.bolt.model.ConnectionState
import xin.cosmos.bolt.model.DeviceUi
import xin.cosmos.bolt.ui.Format
import xin.cosmos.bolt.ui.ManualConnectDialog
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
    val defaultRoot = File(Environment.getExternalStorageDirectory(), "Download/Bolt")
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
    val state by BtEngine.uiState.collectAsState()
    val scope = rememberCoroutineScope()

    // 发送目标 + 选择器结果处理（选择器在结果回调里才知道目标设备）
    var sendTarget by remember { mutableStateOf<DeviceUi?>(null) }
    var showManualConnect by rememberSaveable { mutableStateOf(false) }
    var showFolderPicker by rememberSaveable { mutableStateOf(false) }
    var pickerMode by rememberSaveable { mutableStateOf(PickerMode.Folders) }

    Column(modifier.fillMaxSize()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.devices_title), style = MaterialTheme.typography.titleMedium)
                Text(
                    text = when {
                        state.scanning -> stringResource(R.string.devices_scanning)
                        state.devices.isEmpty() -> stringResource(R.string.devices_scanning_short)
                        else -> stringResource(R.string.devices_found_count, state.devices.size)
                    },
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            TextButton(onClick = { showManualConnect = true }) { Text(stringResource(R.string.devices_btn_manual_connect)) }
            IconButton(onClick = { BtEngine.probeNetwork() }) {
                if (state.scanning) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(20.dp),
                        strokeWidth = 2.dp,
                    )
                } else {
                    Icon(Icons.Filled.Refresh, contentDescription = stringResource(R.string.devices_cd_refresh))
                }
            }
        }

        // 扫描进度条：发现结果异步到达（数秒），期间平滑展开/折叠反馈
        AnimatedCollapse(visible = state.scanning) {
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
        }

        LazyColumn(
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(state.devices, key = { it.uuid }) { device ->
                DeviceCard(
                    modifier = Modifier.animateItem(),
                    device = device,
                    onToggleConnect = {
                        if (device.connState == ConnectionState.Connected) {
                            BtEngine.disconnect(device.uuid)
                        } else {
                            BtEngine.connect(device.uuid)
                        }
                    },
                    onPickFiles = {
                        sendTarget = device
                        pickerMode = PickerMode.Files
                        showFolderPicker = true
                    },
                    onPickFolder = {
                        sendTarget = device
                        pickerMode = PickerMode.Folders
                        showFolderPicker = true
                    },
                )
            }
            if (state.devices.isEmpty()) {
                item {
                    EmptyStateView(
                        icon = painterResource(R.drawable.ic_devices),
                        title = stringResource(R.string.tab_devices),
                        description = stringResource(R.string.devices_empty_hint),
                        actionText = stringResource(R.string.devices_cd_refresh),
                        onAction = { BtEngine.probeNetwork() },
                    )
                }
            }
        }
    }

    if (showManualConnect) {
        ManualConnectDialog(onDismiss = { showManualConnect = false })
    }

    if (showFolderPicker && sendTarget != null) {
        FolderPickerDialog(
            targetName = sendTarget!!.name,
            mode = pickerMode,
            onDismiss = { showFolderPicker = false },
            onSend = { paths ->
                val target = sendTarget ?: return@FolderPickerDialog
                BtEngine.sendFiles(target.uuid, paths, stagedRoot = null)
            },
        )
    }
}

/** SAF 选择结果 → 暂存复制 → bt_send_files（未连接时核心自动先连）。 */
private suspend fun stageAndSend(
    context: android.content.Context,
    target: DeviceUi,
    uris: List<android.net.Uri>? = null,
    tree: android.net.Uri? = null,
) {
    BtEngine.setStaging(true)
    try {
        val staged = when {
            uris != null -> SendStager.stageFiles(context, uris)
            tree != null -> SendStager.stageTree(context, tree)
            else -> null
        }
        if (staged == null) return
        BtEngine.sendFiles(target.uuid, staged.paths, staged.rootDir)
    } catch (_: Exception) {
        // 复制失败（空间不足等）：暂存目录已在内部清理
    } finally {
        BtEngine.setStaging(false)
    }
}

@Composable
private fun DeviceCard(
    device: DeviceUi,
    onToggleConnect: () -> Unit,
    onPickFiles: () -> Unit,
    onPickFolder: () -> Unit,
    modifier: Modifier = Modifier,
) {
    BtCard(modifier = modifier) {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                val iconRes = when (device.deviceType) {
                    1 -> R.drawable.ic_computer
                    2, 3 -> R.drawable.ic_smartphone
                    else -> R.drawable.ic_devices
                }
                Box(
                    modifier = Modifier
                        .size(40.dp)
                        .background(
                            color = MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.5f),
                            shape = CircleShape,
                        ),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        painter = painterResource(iconRes),
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onPrimaryContainer,
                        modifier = Modifier.size(22.dp),
                    )
                }
                Spacer(modifier = Modifier.width(12.dp))
                Column(
                    modifier = Modifier.weight(1f),
                    verticalArrangement = Arrangement.spacedBy(3.dp),
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        Text(
                            text = device.name,
                            style = MaterialTheme.typography.titleMedium.copy(
                                fontSize = 22.sp,
                                fontWeight = FontWeight.Bold,
                            ),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f, fill = false),
                        )
                        if (device.connState == ConnectionState.Connected) {
                            val badgeText = if (device.transport.isNotEmpty()) {
                                stringResource(R.string.devices_connected_protocol, device.transport.uppercase())
                            } else {
                                stringResource(R.string.devices_status_connected)
                            }
                            Box(
                                modifier = Modifier
                                    .background(
                                        color = Color(0xFF10B981).copy(alpha = 0.12f),
                                        shape = CircleShape,
                                    )
                                    .padding(horizontal = 6.dp, vertical = 1.5.dp),
                                contentAlignment = Alignment.Center,
                            ) {
                                Text(
                                    text = badgeText,
                                    color = Color(0xFF10B981),
                                    fontSize = 10.sp,
                                    fontWeight = FontWeight.Normal,
                                    lineHeight = 12.sp,
                                )
                            }
                        }
                    }
                    Text(
                        text = "${stringResource(Format.deviceTypeResId(device.deviceType))} · " +
                            "${device.ip}:${device.quicPort} · ${device.source}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                TextButton(
                    onClick = onToggleConnect,
                    colors = if (device.connState == ConnectionState.Connected) {
                        ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error)
                    } else {
                        ButtonDefaults.textButtonColors()
                    },
                ) {
                    Text(
                        if (device.connState == ConnectionState.Connected) {
                            stringResource(R.string.devices_btn_disconnect)
                        } else {
                            stringResource(R.string.devices_btn_connect)
                        }
                    )
                }
            }
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(top = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedButton(onClick = onPickFiles, modifier = Modifier.weight(1f)) {
                    Text(stringResource(R.string.devices_btn_send_files))
                }
                OutlinedButton(onClick = onPickFolder, modifier = Modifier.weight(1f)) {
                    Text(stringResource(R.string.devices_btn_send_folder))
                }
            }
        }
    }
}
