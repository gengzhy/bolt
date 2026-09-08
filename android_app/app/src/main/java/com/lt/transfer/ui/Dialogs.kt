package com.lt.transfer.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.lt.transfer.engine.LtEngine
import com.lt.transfer.model.ErrorMessages
import com.lt.transfer.model.PendingDialog

/** 弹窗路由：配对请求 / 入站传输请求。 */
@Composable
fun DialogHost(dialog: PendingDialog) {
    when (dialog) {
        is PendingDialog.PairRequest -> PairRequestDialog(dialog)
        is PendingDialog.TransferRequest -> TransferRequestDialog(dialog)
    }
}

/**
 * 配对弹窗（EVT_PAIR_REQUEST）：展示 6 位验证码，
 * 发起端仅等待对端核对确认（配动画与取消），接收端核对一致后接受。
 */
@Composable
private fun PairRequestDialog(dialog: PendingDialog.PairRequest) {
    if (dialog.isInitiator) {
        AlertDialog(
            onDismissRequest = { LtEngine.respondPair(dialog.pairId, false) },
            title = {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(20.dp),
                        strokeWidth = 2.5.dp,
                    )
                    Spacer(Modifier.width(10.dp))
                    Text("设备配对中...")
                }
            },
            text = {
                Column {
                    Text("正在与「${dialog.name}」建立配对连接")
                    Spacer(Modifier.height(12.dp))
                    Text(
                        text = dialog.code,
                        fontSize = 32.sp,
                        fontFamily = FontFamily.Monospace,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    Spacer(Modifier.height(8.dp))
                    Text("请在对方设备屏幕上核对此 6 位验证码并确认接受。")
                }
            },
            confirmButton = {},
            dismissButton = {
                TextButton(onClick = { LtEngine.respondPair(dialog.pairId, false) }) {
                    Text("取消")
                }
            },
        )
    } else {
        AlertDialog(
            onDismissRequest = { LtEngine.respondPair(dialog.pairId, false) },
            title = { Text("配对请求") },
            text = {
                Column {
                    Text("「${dialog.name}」请求与本机配对")
                    Spacer(Modifier.height(12.dp))
                    Text(
                        text = dialog.code,
                        fontSize = 32.sp,
                        fontFamily = FontFamily.Monospace,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    Spacer(Modifier.height(8.dp))
                    Text("请与对方屏幕上显示的验证码比对，一致后接受。")
                }
            },
            confirmButton = {
                TextButton(onClick = { LtEngine.respondPair(dialog.pairId, true) }) {
                    Text("验证码一致，接受")
                }
            },
            dismissButton = {
                TextButton(onClick = { LtEngine.respondPair(dialog.pairId, false) }) {
                    Text("拒绝")
                }
            },
        )
    }
}

/** 入站传输弹窗（EVT_TRANSFER_REQUEST）：展示文件数与总大小。 */
@Composable
private fun TransferRequestDialog(dialog: PendingDialog.TransferRequest) {
    AlertDialog(
        onDismissRequest = { LtEngine.respondTransfer(dialog.reqId, false) },
        title = { Text("接收文件") },
        text = {
            Text(
                "「${dialog.name}」要发送 ${dialog.fileCount} 个文件" +
                    "（共 ${Format.bytes(dialog.totalSize)}）给本机，是否接收？",
            )
        },
        confirmButton = {
            TextButton(onClick = { LtEngine.respondTransfer(dialog.reqId, true) }) {
                Text("接收")
            }
        },
        dismissButton = {
            TextButton(onClick = { LtEngine.respondTransfer(dialog.reqId, false) }) {
                Text("拒绝")
            }
        },
    )
}

/** SAF 暂存复制进行中。 */
@Composable
fun StagingDialog() {
    AlertDialog(
        onDismissRequest = {},
        confirmButton = {},
        text = {
            Column {
                CircularProgressIndicator()
                Spacer(Modifier.height(12.dp))
                Text("正在准备文件（复制到发送暂存区）…")
            }
        },
    )
}

/** 引擎初始化失败（lt_init 返回非 0）。 */
@Composable
fun InitErrorDialog(code: Int) {
    AlertDialog(
        onDismissRequest = {},
        title = { Text("初始化失败") },
        text = { Text("Rust 核心初始化失败（错误码 $code）：${ErrorMessages.of(code)}") },
        confirmButton = {},
    )
}

/** 手动 IP 直连弹窗（需求 §2.1.4 兜底通道）。 */
@Composable
fun ManualConnectDialog(onDismiss: () -> Unit) {
    var ip by rememberSaveable { mutableStateOf("") }
    var port by rememberSaveable { mutableStateOf("8899") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("手动输入 IP 连接") },
        text = {
            Column {
                OutlinedTextField(
                    value = ip,
                    onValueChange = { ip = it },
                    label = { Text("目标 IPv4 地址") },
                    singleLine = true,
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = port,
                    onValueChange = { port = it },
                    label = { Text("端口（默认 8899）") },
                    singleLine = true,
                )
            }
        },
        confirmButton = {
            TextButton(
                enabled = ip.isNotBlank() && (port.toIntOrNull() ?: 0) in 1..65535,
                onClick = {
                    LtEngine.connectAddr(ip.trim(), port.trim().toInt())
                    onDismiss()
                },
            ) { Text("连接") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("取消") } },
    )
}
