package xin.cosmos.bolt.ui.components.dialogs

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xin.cosmos.bolt.R

/**
 * 通用二次确认弹窗（例如清空历史记录、断开连接确认）：
 * 支持配置破坏性操作按钮颜色（isDestructive 为 true 时确认按钮呈警告红色）。
 */
@Composable
fun BtConfirmDialog(
    title: String,
    message: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    confirmText: String = stringResource(R.string.common_confirm),
    dismissText: String = stringResource(R.string.common_cancel),
    isDestructive: Boolean = false,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
            )
        },
        text = {
            Text(
                text = message,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        },
        confirmButton = {
            Button(
                onClick = onConfirm,
                shape = RoundedCornerShape(10.dp),
                colors = if (isDestructive) {
                    ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error)
                } else {
                    ButtonDefaults.buttonColors()
                },
            ) {
                Text(confirmText)
            }
        },
        dismissButton = {
            TextButton(
                onClick = onDismiss,
                shape = RoundedCornerShape(10.dp),
            ) {
                Text(dismissText)
            }
        },
        shape = RoundedCornerShape(18.dp),
        modifier = modifier,
    )
}

/**
 * 通用单字段输入/修改弹窗（修改设备名、端口等）：
 * 自动聚焦输入框、支持回车直接确认、输入校验。
 */
@Composable
fun BtValueEditDialog(
    title: String,
    initialValue: String,
    label: String,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    isNumber: Boolean = false,
    validator: ((String) -> Boolean)? = null,
    confirmText: String = stringResource(R.string.common_confirm),
    dismissText: String = stringResource(R.string.common_cancel),
) {
    var text by remember { mutableStateOf(initialValue) }
    val focusRequester = remember { FocusRequester() }
    val isValid = validator?.invoke(text) ?: text.isNotBlank()

    LaunchedEffect(Unit) {
        focusRequester.requestFocus()
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
            )
        },
        text = {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                label = { Text(label) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(
                    keyboardType = if (isNumber) KeyboardType.Number else KeyboardType.Text,
                    imeAction = ImeAction.Done,
                ),
                keyboardActions = KeyboardActions(
                    onDone = {
                        if (isValid) onConfirm(text)
                    },
                ),
                shape = RoundedCornerShape(12.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .focusRequester(focusRequester),
            )
        },
        confirmButton = {
            Button(
                onClick = { if (isValid) onConfirm(text) },
                enabled = isValid,
                shape = RoundedCornerShape(10.dp),
            ) {
                Text(confirmText)
            }
        },
        dismissButton = {
            TextButton(
                onClick = onDismiss,
                shape = RoundedCornerShape(10.dp),
            ) {
                Text(dismissText)
            }
        },
        shape = RoundedCornerShape(18.dp),
        modifier = modifier,
    )
}

/**
 * 4 位配对验证码独立卡槽展示组件：
 * 将配对验证码拆解为 4 个独立方格包裹数字，舒展大气，居中展示。
 */
@Composable
fun PinCodeDisplay(
    pin: String,
    modifier: Modifier = Modifier,
) {
    val chars = pin.take(4).padEnd(4, ' ').toCharArray()

    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally),
    ) {
        chars.forEach { char ->
            Box(
                modifier = Modifier
                    .size(54.dp, 62.dp)
                    .background(
                        color = MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.28f),
                        shape = RoundedCornerShape(12.dp),
                    )
                    .border(
                        width = 1.5.dp,
                        color = MaterialTheme.colorScheme.primary.copy(alpha = 0.55f),
                        shape = RoundedCornerShape(12.dp),
                    ),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = char.toString(),
                    style = MaterialTheme.typography.headlineMedium.copy(
                        fontFamily = FontFamily.Monospace,
                        fontWeight = FontWeight.Bold,
                    ),
                    color = MaterialTheme.colorScheme.primary,
                    textAlign = TextAlign.Center,
                )
            }
        }
    }
}

/**
 * 隐私政策与数据安全声明弹窗：
 * 展示纯局域网、零服务器、零第三方追踪、TLS 1.3 传输加密与系统权限说明。
 */
@Composable
fun BtPrivacyPolicyDialog(
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Surface(
                    shape = RoundedCornerShape(10.dp),
                    color = MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.6f),
                    modifier = Modifier.size(36.dp),
                ) {
                    Box(contentAlignment = Alignment.Center) {
                        Icon(
                            painter = painterResource(R.drawable.ic_transfers),
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                }
                Column {
                    Text(
                        text = stringResource(R.string.privacy_policy_dialog_title),
                        style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.Bold),
                    )
                    Text(
                        text = stringResource(R.string.privacy_policy_dialog_subtitle),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(max = 420.dp)
                    .verticalScroll(rememberScrollState())
                    .padding(vertical = 4.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                // 核心承诺卡片
                Surface(
                    shape = RoundedCornerShape(10.dp),
                    color = MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.25f),
                    border = BorderStroke(1.dp, MaterialTheme.colorScheme.primary.copy(alpha = 0.35f)),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Column(modifier = Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(
                            text = "核心隐私承诺",
                            style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.Bold),
                            color = MaterialTheme.colorScheme.primary,
                        )
                        Text(
                            text = "Bolt 是一款纯局域网点对点（P2P）开源文件传输工具。无中央服务器，不收集、不上报、不存储任何个人隐私文件。所有传输仅在您局域网授权的两台设备之间直接直连。",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                    }
                }

                PrivacySection(
                    title = "1. 零云端与无第三方追踪",
                    content = "• 零云端服务器：无用户账户系统，无云存储，从根源杜绝外网数据泄露风险。\n• 纯局域网传输：文件传输直接在两台设备间传输，不经过外部互联网。\n• 零第三方追踪：不包含任何商业广告、统计分析或行为追踪类第三方 SDK。",
                )

                PrivacySection(
                    title = "2. 设备信息与传输数据",
                    content = "• 设备信息：设备自定义昵称、局域网内网 IP 与端口，仅在局域网内广播以便双方设备互相发现。\n• 证书指纹：由设备随机生成的 Ed25519 自签名公钥指纹，用于 TLS 1.3 认证与 4 位配对码核对，绝不包含硬件敏感隐私。\n• 文件安全：收发双方直连传输，传输中暂存为 .bttmp 临时文件，完整性校验成功后自动更名。",
                )

                PrivacySection(
                    title = "3. 系统权限申请与使用场景",
                    content = "• 存储访问权限：用于选取要发送的本地文件，以及将接收文件写入保存至您设置的下载目录。\n• 网络与多播权限：用于局域网建立 TCP/QUIC 传输通道，以及 mDNS 服务广播自动发现局域网设备。\n• 通知权限：用于大文件后台传输时，实时展示传输进度与完成状态。",
                )

                PrivacySection(
                    title = "4. 数据安全保障机制",
                    content = "• TLS 1.3 强制加密：通信链路全程强制加密，无明文降级模式，防局域网窃听。\n• TOFU 首次信任：首次配对需核对 4 位动态验证码，信任后锁定指纹，证书变动即刻拦截。\n• BLAKE3 全文哈希校验：传输完成后进行哈希一致性比对，确保文件完整无损。",
                )

                PrivacySection(
                    title = "5. 用户数据自主权",
                    content = "• 信任设备与配置均保存在设备本地，您可随时在设置中清理已信任设备。\n• 系统设置中清除应用数据或直接卸载应用，即可彻底销毁本应用在设备上的所有记录。",
                )

                PrivacySection(
                    title = "6. 联系与开源仓库",
                    content = "Bolt 为开源软件项目，所有源代码均公开透明可查。\n联系邮箱：genggzy@gmail.com",
                )
            }
        },
        confirmButton = {
            Button(
                onClick = onDismiss,
                shape = RoundedCornerShape(10.dp),
            ) {
                Text(stringResource(R.string.privacy_policy_dialog_btn))
            }
        },
        shape = RoundedCornerShape(18.dp),
        modifier = modifier,
    )
}

@Composable
private fun PrivacySection(
    title: String,
    content: String,
) {
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(
            text = title,
            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Bold),
            color = MaterialTheme.colorScheme.primary,
        )
        Text(
            text = content,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            lineHeight = 18.sp,
        )
    }
}

