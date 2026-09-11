package com.lt.transfer.ui.components.form

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * 分区大标题：带有主题色科技感竖条指示器。
 */
@Composable
fun LtSettingSectionHeader(title: String, modifier: Modifier = Modifier) {
    Row(
        modifier = modifier.padding(horizontal = 4.dp, vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
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
        Spacer(Modifier.width(8.dp))
        Text(
            text = title,
            style = MaterialTheme.typography.labelLarge.copy(
                fontWeight = FontWeight.Bold,
                letterSpacing = 0.5.sp,
            ),
            color = MaterialTheme.colorScheme.primary,
        )
    }
}

/**
 * 分区卡片内部分割线。
 */
@Composable
fun LtSettingDivider(modifier: Modifier = Modifier) {
    HorizontalDivider(
        modifier = modifier.padding(horizontal = 16.dp),
        thickness = 0.6.dp,
        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.2f),
    )
}

/**
 * 通用设置行组件：
 * 第一行：左侧标题 + 右侧数值/控件；
 * 第二行：补充说明文案（淡色、自适应换行）。
 */
@Composable
fun LtSettingRow(
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
        // 第一行：左侧参数名称（保底最小宽度，不随右侧超长挤压；设置上限防止撑爆），右侧参数值/控件（自适应占用剩余空间并折行）
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = if (alignTop) Alignment.Top else Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.SemiBold),
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier
                    .widthIn(min = 90.dp, max = 200.dp)
                    .then(if (alignTop) Modifier.padding(top = 4.dp) else Modifier),
            )
            Spacer(Modifier.width(10.dp))
            Box(
                modifier = Modifier
                    .widthIn(min = 72.dp)
                    .weight(1f, fill = false),
                contentAlignment = Alignment.CenterEnd,
            ) {
                trailing()
            }
        }
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
 * 通用开关配置行：整行支持点击触发开关。
 */
@Composable
fun LtSwitchRow(
    title: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
) {
    LtSettingRow(
        title = title,
        subtitle = subtitle,
        onClick = { onCheckedChange(!checked) },
        modifier = modifier,
    ) {
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
        )
    }
}

/** 下拉选择器选项定义 */
data class LtDropdownOption<T>(
    val key: T,
    val label: String,
)

/**
 * 通用下拉选择行：封装下拉浮层、选项展开与当前值渲染。
 */
@Composable
fun <T> LtDropdownPicker(
    title: String,
    options: List<LtDropdownOption<T>>,
    selectedKey: T,
    onSelect: (T) -> Unit,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    isAccent: Boolean = false,
) {
    var expanded by remember { mutableStateOf(false) }
    val currentOption = options.firstOrNull { it.key == selectedKey }
    val currentLabel = currentOption?.label ?: ""

    LtSettingRow(
        title = title,
        subtitle = subtitle,
        onClick = { expanded = true },
        modifier = modifier,
    ) {
        Box {
            LtValueBadge(
                text = "$currentLabel ▾",
                accent = isAccent,
                onClick = { expanded = true },
            )
            DropdownMenu(
                expanded = expanded,
                onDismissRequest = { expanded = false },
            ) {
                options.forEach { option ->
                    DropdownMenuItem(
                        text = {
                            Text(
                                text = option.label,
                                fontWeight = if (option.key == selectedKey) FontWeight.Bold else FontWeight.Normal,
                                color = if (option.key == selectedKey) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
                            )
                        },
                        onClick = {
                            expanded = false
                            if (option.key != selectedKey) {
                                onSelect(option.key)
                            }
                        },
                    )
                }
            }
        }
    }
}

/**
 * 标准参数值胶囊徽章。
 */
@Composable
fun LtValueBadge(
    text: String,
    modifier: Modifier = Modifier,
    accent: Boolean = false,
    mono: Boolean = false,
    onClick: (() -> Unit)? = null,
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
