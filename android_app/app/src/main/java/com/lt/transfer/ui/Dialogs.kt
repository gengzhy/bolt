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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import com.lt.transfer.R
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
                    Text(stringResource(R.string.dialog_pair_initiator_title))
                }
            },
            text = {
                Column {
                    Text(stringResource(R.string.dialog_pair_initiator_msg, dialog.name))
                    Spacer(Modifier.height(12.dp))
                    Text(
                        text = dialog.code,
                        fontSize = 32.sp,
                        fontFamily = FontFamily.Monospace,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    Spacer(Modifier.height(8.dp))
                    Text(stringResource(R.string.dialog_pair_initiator_hint))
                }
            },
            confirmButton = {},
            dismissButton = {
                TextButton(onClick = { LtEngine.respondPair(dialog.pairId, false) }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    } else {
        AlertDialog(
            onDismissRequest = { LtEngine.respondPair(dialog.pairId, false) },
            title = { Text(stringResource(R.string.dialog_pair_receiver_title)) },
            text = {
                Column {
                    Text(stringResource(R.string.dialog_pair_receiver_msg, dialog.name))
                    Spacer(Modifier.height(12.dp))
                    Text(
                        text = dialog.code,
                        fontSize = 32.sp,
                        fontFamily = FontFamily.Monospace,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    Spacer(Modifier.height(8.dp))
                    Text(stringResource(R.string.dialog_pair_receiver_hint))
                }
            },
            confirmButton = {
                TextButton(onClick = { LtEngine.respondPair(dialog.pairId, true) }) {
                    Text(stringResource(R.string.dialog_pair_btn_accept))
                }
            },
            dismissButton = {
                TextButton(onClick = { LtEngine.respondPair(dialog.pairId, false) }) {
                    Text(stringResource(R.string.common_reject))
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
        title = { Text(stringResource(R.string.dialog_transfer_title)) },
        text = {
            Text(
                stringResource(
                    R.string.dialog_transfer_msg,
                    dialog.name,
                    dialog.fileCount,
                    Format.bytes(dialog.totalSize),
                ),
            )
        },
        confirmButton = {
            TextButton(onClick = { LtEngine.respondTransfer(dialog.reqId, true) }) {
                Text(stringResource(R.string.common_accept))
            }
        },
        dismissButton = {
            TextButton(onClick = { LtEngine.respondTransfer(dialog.reqId, false) }) {
                Text(stringResource(R.string.common_reject))
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
                Text(stringResource(R.string.dialog_staging_msg))
            }
        },
    )
}

/** 引擎初始化失败（lt_init 返回非 0）。 */
@Composable
fun InitErrorDialog(code: Int) {
    val context = LocalContext.current
    AlertDialog(
        onDismissRequest = {},
        title = { Text(stringResource(R.string.dialog_init_error_title)) },
        text = { Text(stringResource(R.string.dialog_init_error_msg, code, ErrorMessages.of(context, code))) },
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
        title = { Text(stringResource(R.string.dialog_manual_ip_title)) },
        text = {
            Column {
                OutlinedTextField(
                    value = ip,
                    onValueChange = { ip = it },
                    label = { Text(stringResource(R.string.dialog_manual_ip_label)) },
                    singleLine = true,
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = port,
                    onValueChange = { port = it },
                    label = { Text(stringResource(R.string.dialog_manual_port_label)) },
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
            ) { Text(stringResource(R.string.dialog_btn_connect)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.common_cancel)) } },
    )
}
