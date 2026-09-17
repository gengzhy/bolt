package xin.cosmos.bolt.ui.transfers

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xin.cosmos.bolt.R
import xin.cosmos.bolt.data.TransferLogRepository
import xin.cosmos.bolt.model.TaskStates
import xin.cosmos.bolt.model.TransferLogUi
import xin.cosmos.bolt.ui.Format
import xin.cosmos.bolt.ui.components.dialogs.BtConfirmDialog
import xin.cosmos.bolt.ui.components.feedback.EmptyStateView
import xin.cosmos.bolt.ui.components.feedback.StatusBadge
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * 传输记录日志页面（独立页面）：
 * 展示传输历史审计记录，包含：
 * - 发送/接收
 * - 协议名称 (QUIC / TCP)
 * - 文件名称
 * - 文件大小
 * - 传输速率
 * - 传输开始 / 结束时间与耗时
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TransferLogsScreen(
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    BackHandler(onBack = onBack)

    val logs by TransferLogRepository.logs.collectAsState()
    var selectedFilter by rememberSaveable { mutableIntStateOf(0) } // 0: 全部, 1: 发送, 2: 接收
    var showClearConfirm by remember { mutableStateOf(false) }

    val filteredLogs = remember(logs, selectedFilter) {
        when (selectedFilter) {
            1 -> logs.filter { !it.incoming }
            2 -> logs.filter { it.incoming }
            else -> logs
        }
    }

    Scaffold(
        modifier = modifier.fillMaxSize(),
        topBar = {
            TopAppBar(
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "返回",
                        )
                    }
                },
                title = {
                    Text(
                        text = stringResource(R.string.transfer_logs_title),
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.Bold,
                    )
                },
                actions = {
                    if (logs.isNotEmpty()) {
                        TextButton(onClick = { showClearConfirm = true }) {
                            Icon(
                                imageVector = Icons.Filled.Delete,
                                contentDescription = null,
                                modifier = Modifier.size(18.dp),
                            )
                            Spacer(Modifier.width(4.dp))
                            Text(stringResource(R.string.transfer_logs_btn_clear))
                        }
                    }
                },
            )
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding),
        ) {
            // 顶部过滤条 (全部 / 发送 / 接收)
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                val sendCount = logs.count { !it.incoming }
                val recvCount = logs.count { it.incoming }

                FilterChip(
                    selected = selectedFilter == 0,
                    onClick = { selectedFilter = 0 },
                    label = { Text(stringResource(R.string.transfer_logs_filter_all, logs.size)) },
                    colors = FilterChipDefaults.filterChipColors(
                        selectedContainerColor = MaterialTheme.colorScheme.primaryContainer,
                        selectedLabelColor = MaterialTheme.colorScheme.onPrimaryContainer,
                    ),
                )
                FilterChip(
                    selected = selectedFilter == 1,
                    onClick = { selectedFilter = 1 },
                    label = { Text(stringResource(R.string.transfer_logs_filter_send, sendCount)) },
                    colors = FilterChipDefaults.filterChipColors(
                        selectedContainerColor = MaterialTheme.colorScheme.primaryContainer,
                        selectedLabelColor = MaterialTheme.colorScheme.onPrimaryContainer,
                    ),
                )
                FilterChip(
                    selected = selectedFilter == 2,
                    onClick = { selectedFilter = 2 },
                    label = { Text(stringResource(R.string.transfer_logs_filter_recv, recvCount)) },
                    colors = FilterChipDefaults.filterChipColors(
                        selectedContainerColor = MaterialTheme.colorScheme.primaryContainer,
                        selectedLabelColor = MaterialTheme.colorScheme.onPrimaryContainer,
                    ),
                )
            }

            // 日志卡片列表
            LazyColumn(
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
                modifier = Modifier.fillMaxSize(),
            ) {
                items(filteredLogs, key = { it.id }) { log ->
                    TransferLogCard(log = log)
                }

                if (filteredLogs.isEmpty()) {
                    item {
                        EmptyStateView(
                            icon = painterResource(R.drawable.ic_transfers),
                            title = stringResource(R.string.transfer_logs_empty),
                            description = stringResource(R.string.transfer_logs_empty_desc),
                        )
                    }
                }
            }
        }
    }

    if (showClearConfirm) {
        BtConfirmDialog(
            title = stringResource(R.string.transfer_logs_confirm_clear_title),
            message = stringResource(R.string.transfer_logs_confirm_clear_msg),
            isDestructive = true,
            onConfirm = {
                showClearConfirm = false
                TransferLogRepository.clearLogs()
            },
            onDismiss = { showClearConfirm = false },
        )
    }
}

@Composable
private fun TransferLogCard(log: TransferLogUi) {
    val dateFormat = remember { SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.getDefault()) }

    Card(
        shape = RoundedCornerShape(14.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.45f)),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(modifier = Modifier.padding(14.dp)) {
            // 1. 顶部 Header：方向 Badge + 协议 Badge + 对端设备名 + 状态 Badge
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth(),
            ) {
                // 方向 Badge
                val isSend = !log.incoming
                val dirBg = if (isSend) MaterialTheme.colorScheme.primaryContainer else Color(0xFFE8F5E9)
                val dirColor = if (isSend) MaterialTheme.colorScheme.primary else Color(0xFF2E7D32)
                val dirText = if (isSend) stringResource(R.string.transfers_dir_outgoing) else stringResource(R.string.transfers_dir_incoming)

                Box(
                    modifier = Modifier
                        .background(dirBg, RoundedCornerShape(6.dp))
                        .padding(horizontal = 8.dp, vertical = 3.dp),
                ) {
                    Text(
                        text = dirText,
                        color = dirColor,
                        style = MaterialTheme.typography.labelSmall,
                        fontWeight = FontWeight.Bold,
                    )
                }

                Spacer(Modifier.width(8.dp))

                // 协议 Badge (QUIC / TCP)
                if (log.transport.isNotBlank()) {
                    Box(
                        modifier = Modifier
                            .background(MaterialTheme.colorScheme.tertiaryContainer, RoundedCornerShape(6.dp))
                            .padding(horizontal = 6.dp, vertical = 3.dp),
                    ) {
                        Text(
                            text = log.transport.uppercase(),
                            color = MaterialTheme.colorScheme.onTertiaryContainer,
                            style = MaterialTheme.typography.labelSmall,
                            fontWeight = FontWeight.Bold,
                        )
                    }
                    Spacer(Modifier.width(8.dp))
                }

                // 对端名称
                val peer = log.peerName.ifEmpty { log.peerUuid.take(8) }
                if (peer.isNotBlank()) {
                    Text(
                        text = "· $peer",
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = FontWeight.Medium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                } else {
                    Spacer(Modifier.weight(1f))
                }

                // 状态 Badge
                val stateResId = Format.stateResId(log.state)
                val stateText = if (stateResId != 0) stringResource(stateResId) else log.state
                val stateColor = when (log.state) {
                    TaskStates.DONE -> MaterialTheme.colorScheme.primary
                    TaskStates.ERROR -> MaterialTheme.colorScheme.error
                    TaskStates.CANCELLED, TaskStates.REJECTED -> MaterialTheme.colorScheme.outline
                    else -> MaterialTheme.colorScheme.tertiary
                }
                StatusBadge(
                    text = stateText,
                    color = stateColor,
                )
            }

            Spacer(Modifier.height(10.dp))

            // 2. 文件信息：文件图标 + 文件名称
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Icon(
                    painter = painterResource(if (log.fileCount > 1) R.drawable.ic_folder else R.drawable.ic_file),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(22.dp),
                )
                Spacer(Modifier.width(8.dp))
                Text(
                    text = log.fileName,
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            Spacer(Modifier.height(10.dp))

            // 3. 指标行：文件大小 · 传输速率 · 传输耗时
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                val sizeText = Format.bytes(log.totalSize)
                val rateText = if (log.avgRateBps > 0) Format.bytes(log.avgRateBps) + "/s" else "—"
                val durationText = if (log.durationMs > 0) {
                    if (log.durationMs < 1000) {
                        "${log.durationMs} ms"
                    } else {
                        String.format(Locale.getDefault(), "%.1f s", log.durationMs / 1000.0)
                    }
                } else {
                    "—"
                }

                Column {
                    Text(
                        text = "文件大小",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                        fontSize = 11.sp,
                    )
                    Text(
                        text = sizeText,
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = FontWeight.SemiBold,
                    )
                }

                Column {
                    Text(
                        text = "传输速率",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                        fontSize = 11.sp,
                    )
                    Text(
                        text = rateText,
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.primary,
                    )
                }

                Column {
                    Text(
                        text = "传输耗时",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                        fontSize = 11.sp,
                    )
                    Text(
                        text = durationText,
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = FontWeight.SemiBold,
                    )
                }
            }

            Spacer(Modifier.height(10.dp))

            // 4. 起止时间展示
            val startFormatted = if (log.startTimeMs > 0) dateFormat.format(Date(log.startTimeMs)) else "—"
            val endFormatted = when {
                log.endTimeMs > 0 -> dateFormat.format(Date(log.endTimeMs))
                TaskStates.isActive(log.state) -> stringResource(R.string.transfer_logs_ongoing)
                else -> "—"
            }

            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(MaterialTheme.colorScheme.surface.copy(alpha = 0.6f), RoundedCornerShape(8.dp))
                    .padding(horizontal = 10.dp, vertical = 6.dp),
                verticalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                Text(
                    text = stringResource(R.string.transfer_logs_field_start_time, startFormatted),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 11.sp,
                )
                Text(
                    text = stringResource(R.string.transfer_logs_field_end_time, endFormatted),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 11.sp,
                )
            }
        }
    }
}
