package xin.cosmos.bolt.ui.devices

import android.widget.Toast
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.launch
import xin.cosmos.bolt.R
import xin.cosmos.bolt.engine.BtEngine
import xin.cosmos.bolt.engine.SendStager
import xin.cosmos.bolt.engine.SharePayload
import xin.cosmos.bolt.engine.SharePayloadHelper
import xin.cosmos.bolt.model.ConnectionState
import xin.cosmos.bolt.model.DeviceUi
import xin.cosmos.bolt.ui.ManualConnectDialog

/**
 * 系统分享快速发送弹窗：
 * 当通过系统分享面板选择 Bolt 时弹出，展示待发送文件概况和当前局域网在线设备列表，
 * 点击任一设备即可一键暂存并极速开始发送。
 */
@Composable
fun QuickShareDialog(
    payload: SharePayload,
    onDismiss: () -> Unit,
    onSendSuccess: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val state by BtEngine.uiState.collectAsState()

    var isStaging by remember { mutableStateOf(false) }
    var showManualConnect by remember { mutableStateOf(false) }

    // 在线设备列表：已连接设备优先展示，其次按设备名称排序
    val deviceList: List<DeviceUi> = remember(state.devices) {
        state.devices.sortedWith(
            compareByDescending<DeviceUi> { it.connState == ConnectionState.Connected }
                .thenBy { it.name.lowercase() }
        )
    }

    Dialog(
        onDismissRequest = {
            if (!isStaging) {
                SharePayloadHelper.clear()
                onDismiss()
            }
        },
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Card(
            modifier = Modifier
                .fillMaxWidth(0.92f)
                .fillMaxHeight(0.72f),
            shape = RoundedCornerShape(24.dp),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
            elevation = CardDefaults.cardElevation(defaultElevation = 8.dp),
        ) {
            Column(modifier = Modifier.fillMaxSize()) {
                // 1. 顶部标题条
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        painter = painterResource(R.drawable.ic_transfers),
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(24.dp),
                    )
                    Spacer(modifier = Modifier.width(10.dp))
                    Text(
                        text = stringResource(R.string.quick_share_title),
                        style = MaterialTheme.typography.titleLarge.copy(
                            fontWeight = FontWeight.Bold,
                            fontSize = 20.sp,
                        ),
                        modifier = Modifier.weight(1f),
                    )
                    IconButton(
                        onClick = {
                            if (!isStaging) {
                                SharePayloadHelper.clear()
                                onDismiss()
                            }
                        },
                        enabled = !isStaging,
                    ) {
                        Icon(
                            imageVector = Icons.Filled.Close,
                            contentDescription = stringResource(R.string.common_close),
                        )
                    }
                }

                // 2. 待发送文件概况条
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp)
                        .clip(RoundedCornerShape(14.dp))
                        .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f))
                        .padding(horizontal = 14.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(
                        modifier = Modifier
                            .size(36.dp)
                            .clip(CircleShape)
                            .background(MaterialTheme.colorScheme.primary.copy(alpha = 0.12f)),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            painter = painterResource(R.drawable.ic_file),
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                    Spacer(modifier = Modifier.width(12.dp))
                    Text(
                        text = payload.summaryText,
                        style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Medium),
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                        color = MaterialTheme.colorScheme.onSurface,
                        modifier = Modifier.weight(1f),
                    )
                }

                Spacer(modifier = Modifier.height(12.dp))
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))

                // 3. 设备列表区域
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth(),
                ) {
                    if (isStaging) {
                        // 正在暂存文件加载状态
                        Column(
                            modifier = Modifier
                                .fillMaxSize()
                                .padding(24.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.Center,
                        ) {
                            CircularProgressIndicator(
                                modifier = Modifier.size(36.dp),
                                strokeWidth = 3.dp,
                            )
                            Spacer(modifier = Modifier.height(14.dp))
                            Text(
                                text = stringResource(R.string.quick_share_staging),
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    } else if (deviceList.isEmpty()) {
                        // 暂无发现设备：展示搜索提示与手动连接入口
                        Column(
                            modifier = Modifier
                                .fillMaxSize()
                                .padding(24.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.Center,
                        ) {
                            CircularProgressIndicator(
                                modifier = Modifier.size(32.dp),
                                strokeWidth = 2.5.dp,
                                color = MaterialTheme.colorScheme.primary.copy(alpha = 0.7f),
                            )
                            Spacer(modifier = Modifier.height(14.dp))
                            Text(
                                text = stringResource(R.string.quick_share_no_devices),
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            Spacer(modifier = Modifier.height(16.dp))
                            OutlinedButton(
                                onClick = { showManualConnect = true },
                                shape = RoundedCornerShape(12.dp),
                            ) {
                                Text(stringResource(R.string.quick_share_manual_connect))
                            }
                        }
                    } else {
                        // 展示发现的设备列表
                        LazyColumn(
                            modifier = Modifier.fillMaxSize(),
                            contentPadding = PaddingValues(horizontal = 16.dp, vertical = 10.dp),
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            item {
                                Text(
                                    text = stringResource(R.string.quick_share_select_device_hint),
                                    style = MaterialTheme.typography.labelMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.padding(horizontal = 4.dp, vertical = 4.dp),
                                )
                            }
                            items(
                                items = deviceList,
                                key = { it.uuid },
                            ) { device ->
                                QuickShareDeviceItem(
                                    device = device,
                                    onClick = {
                                        scope.launch {
                                            isStaging = true
                                            BtEngine.setStaging(true)
                                            try {
                                                val staged = SendStager.stageFiles(context, payload.uris)
                                                if (staged != null) {
                                                    BtEngine.sendFiles(device.uuid, staged.paths, staged.rootDir)
                                                    Toast.makeText(
                                                        context,
                                                        R.string.quick_share_started,
                                                        Toast.LENGTH_SHORT,
                                                    ).show()
                                                    SharePayloadHelper.clear()
                                                    onSendSuccess()
                                                    onDismiss()
                                                }
                                            } catch (_: Exception) {
                                                // 失败处理
                                            } finally {
                                                isStaging = false
                                                BtEngine.setStaging(false)
                                            }
                                        }
                                    },
                                )
                            }
                        }
                    }
                }

                // 4. 底部关闭/手动连接栏
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 10.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    TextButton(onClick = { showManualConnect = true }, enabled = !isStaging) {
                        Text(stringResource(R.string.quick_share_manual_connect))
                    }
                    TextButton(
                        onClick = {
                            SharePayloadHelper.clear()
                            onDismiss()
                        },
                        enabled = !isStaging,
                    ) {
                        Text(stringResource(R.string.common_cancel))
                    }
                }
            }
        }
    }

    if (showManualConnect) {
        ManualConnectDialog(onDismiss = { showManualConnect = false })
    }
}

/** 快速分享弹窗中的单个设备卡片 */
@Composable
private fun QuickShareDeviceItem(
    device: DeviceUi,
    onClick: () -> Unit,
) {
    val isConnected = device.connState == ConnectionState.Connected
    val iconRes = when (device.deviceType) {
        1 -> R.drawable.ic_computer
        2, 3 -> R.drawable.ic_smartphone
        else -> R.drawable.ic_devices
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(
                if (isConnected) {
                    MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.35f)
                } else {
                    MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.35f)
                }
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier = Modifier
                .size(42.dp)
                .background(
                    color = if (isConnected) {
                        MaterialTheme.colorScheme.primaryContainer
                    } else {
                        MaterialTheme.colorScheme.surfaceVariant
                    },
                    shape = CircleShape,
                ),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painter = painterResource(iconRes),
                contentDescription = null,
                tint = if (isConnected) {
                    MaterialTheme.colorScheme.primary
                } else {
                    MaterialTheme.colorScheme.onSurfaceVariant
                },
                modifier = Modifier.size(22.dp),
            )
        }
        Spacer(modifier = Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Text(
                    text = device.name,
                    style = MaterialTheme.typography.titleMedium.copy(
                        fontWeight = FontWeight.Bold,
                        fontSize = 17.sp,
                    ),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (isConnected) {
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(4.dp))
                            .background(MaterialTheme.colorScheme.primary.copy(alpha = 0.15f))
                            .padding(horizontal = 6.dp, vertical = 2.dp),
                    ) {
                        Text(
                            text = stringResource(R.string.devices_status_connected),
                            style = MaterialTheme.typography.labelSmall.copy(
                                fontWeight = FontWeight.Bold,
                                fontSize = 11.sp,
                            ),
                            color = MaterialTheme.colorScheme.primary,
                        )
                    }
                }
            }
            Spacer(modifier = Modifier.height(2.dp))
            Text(
                text = "${device.ip}:${device.quicPort}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.8f),
            )
        }
        Spacer(modifier = Modifier.width(8.dp))
        Button(
            onClick = onClick,
            shape = RoundedCornerShape(10.dp),
        ) {
            Text(stringResource(R.string.folder_picker_btn_send), fontWeight = FontWeight.Bold)
        }
    }
}
