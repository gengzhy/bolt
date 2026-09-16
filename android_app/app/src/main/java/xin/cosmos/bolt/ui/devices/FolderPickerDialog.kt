package xin.cosmos.bolt.ui.devices

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.Settings
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import xin.cosmos.bolt.R
import xin.cosmos.bolt.ui.Format
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** 选择器模式：仅选择文件夹 或 仅选择文件。 */
enum class PickerMode {
    Folders,
    Files,
}

/**
 * 文件/文件夹项 UI 数据类：
 * 在后台 IO 线程预先计算好各字段，确保 Compose 渲染和滑动时主线程零阻塞、零磁盘系统调用。
 */
@Immutable
data class FileItemUi(
    val file: File,
    val path: String,
    val name: String,
    val isDirectory: Boolean,
    val sizeText: String = "",
    val modDateText: String = "",
)

/**
 * 原生内置文件/文件夹选择器弹窗：
 * - [PickerMode.Folders] 模式：列表仅展示文件夹，每项前有复选框可供选择，点击整行可深入子目录；
 * - [PickerMode.Files] 模式：列表展示文件夹（点击进入，无复选框）以及具体文件（前面有复选框供勾选）；
 * - 顶部提供路径面包屑导航与层级回退；
 * - 底部显示已勾选计数与【发送】按钮，确定后直接调用引擎零拷贝发送。
 */
@Composable
fun FolderPickerDialog(
    targetName: String,
    mode: PickerMode = PickerMode.Folders,
    onDismiss: () -> Unit,
    onSend: (List<String>) -> Unit,
) {
    val context = LocalContext.current
    val hasAllFilesAccess = remember {
        if (Build.VERSION.SDK_INT >= 30) {
            Environment.isExternalStorageManager()
        } else {
            true
        }
    }

    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Card(
            modifier = Modifier
                .fillMaxWidth(0.95f)
                .fillMaxHeight(0.88f),
            shape = RoundedCornerShape(24.dp),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
            elevation = CardDefaults.cardElevation(defaultElevation = 8.dp),
        ) {
            if (!hasAllFilesAccess) {
                PermissionRequiredView(onDismiss = onDismiss)
                return@Card
            }

            val defaultRoot = remember { Environment.getExternalStorageDirectory() }
            // 初始目录：优先尝试进入 Download 目录便于用户快速选取，不存在则从存储根目录开始
            var currentDir by remember {
                val dl = File(defaultRoot, "Download")
                mutableStateOf(if (dl.exists() && dl.isDirectory && dl.canRead()) dl else defaultRoot)
            }

            // 已勾选的项目绝对路径集合（使用 Set 达到 O(1) 查询效率，避免重组列表卡顿）
            var selectedPaths by remember { mutableStateOf(emptySet<String>()) }

            // 当前目录下的文件夹列表、文件列表与加载状态
            var dirItems by remember { mutableStateOf<List<FileItemUi>>(emptyList()) }
            var fileItems by remember { mutableStateOf<List<FileItemUi>>(emptyList()) }
            var isLoading by remember { mutableStateOf(false) }
            var showLoadingIndicator by remember { mutableStateOf(false) }
            var isInitialLoad by remember { mutableStateOf(true) }
            val listState = rememberLazyListState()

            val listAlpha by animateFloatAsState(
                targetValue = if (showLoadingIndicator) 0.65f else 1f,
                label = "listAlpha",
            )

            // 监听返回键：如果不是在存储根目录，则后退到上一级；在根目录时关闭弹窗
            BackHandler {
                if (currentDir != defaultRoot && currentDir.parentFile != null) {
                    currentDir = currentDir.parentFile!!
                } else {
                    onDismiss()
                }
            }

            // 异步高性能加载：单次遍历、预格式化、首屏秒开、防抖平滑无闪烁
            LaunchedEffect(currentDir, mode) {
                isLoading = true
                // 防抖延迟：对于 99% 耗时仅 3~15ms 的常规目录，不触发任何加载指示器，彻底杜绝瞬间闪烁
                val loadingJob = launch {
                    delay(80)
                    showLoadingIndicator = true
                }

                try {
                    withContext(Dispatchers.IO) {
                        val isAndroidRoot = currentDir == File(defaultRoot, "Android")
                        val rawFiles = currentDir.listFiles()
                        if (rawFiles == null || rawFiles.isEmpty()) {
                            withContext(Dispatchers.Main) {
                                dirItems = emptyList()
                                fileItems = emptyList()
                                listState.scrollToItem(0)
                            }
                            return@withContext
                        }

                        val dList = ArrayList<FileItemUi>()
                        val fList = ArrayList<FileItemUi>()
                        val dateFormat = SimpleDateFormat("yyyy/MM/dd HH:mm", Locale.getDefault())

                        val isLargeDir = rawFiles.size > 120
                        var firstBatchEmitted = false

                        for (f in rawFiles) {
                            val name = f.name
                            if (name.startsWith(".")) continue
                            if (isAndroidRoot && (name == "data" || name == "obb")) continue

                            val isDir = f.isDirectory
                            val lastMod = f.lastModified()
                            val modDate = if (lastMod > 0) dateFormat.format(Date(lastMod)) else ""

                            if (isDir) {
                                dList.add(
                                    FileItemUi(
                                        file = f,
                                        path = f.absolutePath,
                                        name = name,
                                        isDirectory = true,
                                        modDateText = modDate,
                                    )
                                )
                            } else if (mode == PickerMode.Files) {
                                val len = f.length()
                                fList.add(
                                    FileItemUi(
                                        file = f,
                                        path = f.absolutePath,
                                        name = name,
                                        isDirectory = false,
                                        sizeText = Format.bytes(len),
                                        modDateText = modDate,
                                    )
                                )
                            }

                            // 针对上千项的超大文件夹（如 DCIM/Camera），在解析到前 80 项时立即刷新首屏，提供即刻反馈
                            if (isLargeDir && !firstBatchEmitted && (dList.size + fList.size) >= 80) {
                                val initialDirs = ArrayList(dList).apply { sortWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name }) }
                                val initialFiles = ArrayList(fList).apply { sortWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name }) }
                                withContext(Dispatchers.Main) {
                                    dirItems = initialDirs
                                    fileItems = initialFiles
                                    listState.scrollToItem(0)
                                }
                                firstBatchEmitted = true
                            }
                        }

                        dList.sortWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name })
                        if (mode == PickerMode.Files) {
                            fList.sortWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name })
                        }

                        withContext(Dispatchers.Main) {
                            dirItems = dList
                            fileItems = fList
                            if (!firstBatchEmitted) {
                                listState.scrollToItem(0)
                            }
                        }
                    }
                } catch (e: Exception) {
                    if (e is CancellationException) throw e
                    withContext(Dispatchers.Main) {
                        dirItems = emptyList()
                        fileItems = emptyList()
                    }
                } finally {
                    loadingJob.cancel()
                    isLoading = false
                    showLoadingIndicator = false
                    isInitialLoad = false
                }
            }

            Column(modifier = Modifier.fillMaxSize()) {
                // 1. 顶部标题栏
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 8.dp, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    IconButton(
                        onClick = {
                            if (currentDir != defaultRoot && currentDir.parentFile != null) {
                                currentDir = currentDir.parentFile!!
                            } else {
                                onDismiss()
                            }
                        },
                    ) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "返回上一级",
                        )
                    }
                    val titleText = if (mode == PickerMode.Folders) {
                        stringResource(R.string.folder_picker_title)
                    } else {
                        stringResource(R.string.file_picker_title)
                    }
                    Text(
                        text = titleText,
                        style = MaterialTheme.typography.titleLarge.copy(
                            fontWeight = FontWeight.Bold,
                            fontSize = 19.sp,
                        ),
                        modifier = Modifier.weight(1f),
                    )
                    IconButton(onClick = onDismiss) {
                        Icon(
                            imageVector = Icons.Filled.Close,
                            contentDescription = "关闭",
                        )
                    }
                }

                // 2. 路径面包屑导航栏
                PathBreadcrumbs(
                    rootDir = defaultRoot,
                    currentDir = currentDir,
                    onNavigate = { currentDir = it },
                )

                // 路径与列表之间的分割线 + 延时平滑进度条（仅在耗时 > 80ms 时显示，绝不跳变闪烁）
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(2.dp),
                ) {
                    HorizontalDivider(
                        modifier = Modifier.fillMaxSize(),
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                    )
                    if (showLoadingIndicator) {
                        LinearProgressIndicator(
                            modifier = Modifier.fillMaxSize(),
                        )
                    }
                }

                // 3. 列表主体
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth(),
                ) {
                    if (isInitialLoad && showLoadingIndicator && dirItems.isEmpty() && fileItems.isEmpty()) {
                        CircularProgressIndicator(
                            modifier = Modifier
                                .size(36.dp)
                                .align(Alignment.Center),
                            strokeWidth = 3.dp,
                        )
                    } else if (!isLoading && dirItems.isEmpty() && fileItems.isEmpty()) {
                        Column(
                            modifier = Modifier
                                .fillMaxSize()
                                .padding(24.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.Center,
                        ) {
                            val emptyIcon = if (mode == PickerMode.Folders) R.drawable.ic_folder else R.drawable.ic_file
                            Icon(
                                painter = painterResource(emptyIcon),
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f),
                                modifier = Modifier.size(56.dp),
                            )
                            Spacer(modifier = Modifier.height(12.dp))
                            val emptyText = if (mode == PickerMode.Folders) {
                                stringResource(R.string.folder_picker_empty)
                            } else {
                                stringResource(R.string.file_picker_empty)
                            }
                            Text(
                                text = emptyText,
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    } else {
                        LazyColumn(
                            state = listState,
                            modifier = Modifier
                                .fillMaxSize()
                                .alpha(listAlpha),
                            contentPadding = PaddingValues(horizontal = 12.dp, vertical = 8.dp),
                            verticalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            // 文件夹列表项
                            items(
                                items = dirItems,
                                key = { "d_${it.path}" },
                                contentType = { "folder" },
                            ) { item ->
                                FolderListItem(
                                    item = item,
                                    mode = mode,
                                    isSelected = selectedPaths.contains(item.path),
                                    onToggleSelect = { checked ->
                                        selectedPaths = if (checked) selectedPaths + item.path else selectedPaths - item.path
                                    },
                                    onClick = {
                                        currentDir = item.file
                                    },
                                )
                            }

                            // 文件列表项（仅在 Files 模式展示）
                            if (mode == PickerMode.Files) {
                                items(
                                    items = fileItems,
                                    key = { "f_${it.path}" },
                                    contentType = { "file" },
                                ) { item ->
                                    val isSelected = selectedPaths.contains(item.path)
                                    FileListItem(
                                        item = item,
                                        isSelected = isSelected,
                                        onToggleSelect = { checked ->
                                            selectedPaths = if (checked) selectedPaths + item.path else selectedPaths - item.path
                                        },
                                        onClick = {
                                            selectedPaths = if (isSelected) selectedPaths - item.path else selectedPaths + item.path
                                        },
                                    )
                                }
                            }
                        }
                    }
                }

                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))

                // 4. 底部操作栏
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.35f))
                        .padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    val countText = if (mode == PickerMode.Folders) {
                        stringResource(R.string.folder_picker_selected_count, selectedPaths.size)
                    } else {
                        stringResource(R.string.file_picker_selected_count, selectedPaths.size)
                    }
                    Text(
                        text = countText,
                        style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Medium),
                        color = if (selectedPaths.isNotEmpty()) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(onClick = onDismiss) {
                        Text(stringResource(R.string.common_cancel))
                    }
                    Spacer(modifier = Modifier.width(8.dp))
                    Button(
                        onClick = {
                            if (selectedPaths.isNotEmpty()) {
                                onSend(selectedPaths.toList())
                                onDismiss()
                            }
                        },
                        enabled = selectedPaths.isNotEmpty(),
                        shape = RoundedCornerShape(12.dp),
                    ) {
                        val sendText = if (selectedPaths.isNotEmpty()) {
                            stringResource(R.string.folder_picker_btn_send_with_count, selectedPaths.size)
                        } else {
                            stringResource(R.string.folder_picker_btn_send)
                        }
                        Text(sendText, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}

/**
 * 文件夹列表单项：
 * - 在 [PickerMode.Folders] 下：左侧复选框 + 文件夹图标 + 文件夹名及修改日期 + 右侧进入箭头
 * - 在 [PickerMode.Files] 下：无复选框（不可选文件夹），仅用于点击进入下一级
 */
@Composable
private fun FolderListItem(
    item: FileItemUi,
    mode: PickerMode,
    isSelected: Boolean,
    onToggleSelect: (Boolean) -> Unit,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .background(
                if (isSelected) {
                    MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.45f)
                } else {
                    Color.Transparent
                },
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 8.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // 1. 左侧复选框（仅在 Folders 模式提供）
        if (mode == PickerMode.Folders) {
            Checkbox(
                checked = isSelected,
                onCheckedChange = onToggleSelect,
                modifier = Modifier.size(36.dp),
            )
            Spacer(modifier = Modifier.width(4.dp))
        } else {
            Spacer(modifier = Modifier.width(8.dp))
        }

        // 2. 文件夹图标
        Box(
            modifier = Modifier
                .size(40.dp)
                .clip(RoundedCornerShape(10.dp))
                .background(MaterialTheme.colorScheme.primary.copy(alpha = 0.1f)),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_folder),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(24.dp),
            )
        }

        Spacer(modifier = Modifier.width(12.dp))

        // 3. 文件夹名称与修改日期
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = item.name,
                style = MaterialTheme.typography.titleMedium.copy(
                    fontWeight = FontWeight.SemiBold,
                    fontSize = 16.sp,
                ),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (item.modDateText.isNotEmpty()) {
                Spacer(modifier = Modifier.height(2.dp))
                Text(
                    text = item.modDateText,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                )
            }
        }

        // 4. 右侧深入进入箭头
        Icon(
            painter = painterResource(R.drawable.ic_chevron_right),
            contentDescription = "进入目录",
            tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.45f),
            modifier = Modifier
                .padding(end = 4.dp)
                .size(15.dp),
        )
    }
}

/** 文件列表单项（仅在 [PickerMode.Files] 下展示）：左侧复选框 + 文件图标 + 文件名及大小 + 日期 */
@Composable
private fun FileListItem(
    item: FileItemUi,
    isSelected: Boolean,
    onToggleSelect: (Boolean) -> Unit,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .background(
                if (isSelected) {
                    MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.45f)
                } else {
                    Color.Transparent
                },
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 8.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // 1. 左侧复选框
        Checkbox(
            checked = isSelected,
            onCheckedChange = onToggleSelect,
            modifier = Modifier.size(36.dp),
        )

        Spacer(modifier = Modifier.width(4.dp))

        // 2. 文件图标
        Box(
            modifier = Modifier
                .size(40.dp)
                .clip(RoundedCornerShape(10.dp))
                .background(MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.6f)),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_file),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSecondaryContainer,
                modifier = Modifier.size(22.dp),
            )
        }

        Spacer(modifier = Modifier.width(12.dp))

        // 3. 文件名及大小与修改日期
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = item.name,
                style = MaterialTheme.typography.bodyLarge.copy(
                    fontWeight = FontWeight.Normal,
                    fontSize = 15.sp,
                ),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(modifier = Modifier.height(2.dp))
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                if (item.sizeText.isNotEmpty()) {
                    Text(
                        text = item.sizeText,
                        style = MaterialTheme.typography.bodySmall.copy(fontWeight = FontWeight.Medium),
                        color = MaterialTheme.colorScheme.primary,
                    )
                }
                if (item.modDateText.isNotEmpty()) {
                    if (item.sizeText.isNotEmpty()) {
                        Text(
                            text = "·",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f),
                        )
                    }
                    Text(
                        text = item.modDateText,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                    )
                }
            }
        }
    }
}

/** 路径面包屑导航组件 */
@Composable
private fun PathBreadcrumbs(
    rootDir: File,
    currentDir: File,
    onNavigate: (File) -> Unit,
) {
    val context = LocalContext.current
    val segments = remember(currentDir, rootDir) {
        val list = mutableListOf<Pair<String, File>>()
        val rootTitle = context.getString(R.string.folder_picker_root_name)
        list.add(Pair(rootTitle, rootDir))

        if (currentDir != rootDir && currentDir.startsWith(rootDir)) {
            val rel = currentDir.relativeTo(rootDir).path.replace('\\', '/')
            var accum = rootDir
            for (part in rel.split('/').filter { it.isNotEmpty() }) {
                accum = File(accum, part)
                list.add(Pair(part, accum))
            }
        }
        list
    }

    val scrollState = rememberScrollState()
    LaunchedEffect(segments.size) {
        scrollState.animateScrollTo(scrollState.maxValue)
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(scrollState)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        segments.forEachIndexed { index, (name, dir) ->
            val isLast = index == segments.lastIndex
            if (isLast) {
                Text(
                    text = name,
                    style = MaterialTheme.typography.bodyMedium.copy(
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.primary,
                    ),
                    modifier = Modifier
                        .clip(RoundedCornerShape(6.dp))
                        .background(MaterialTheme.colorScheme.primary.copy(alpha = 0.08f))
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                )
            } else {
                Text(
                    text = name,
                    style = MaterialTheme.typography.bodyMedium.copy(
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    ),
                    modifier = Modifier
                        .clip(RoundedCornerShape(6.dp))
                        .clickable { onNavigate(dir) }
                        .padding(horizontal = 6.dp, vertical = 4.dp),
                )
                Icon(
                    painter = painterResource(R.drawable.ic_chevron_right),
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f),
                    modifier = Modifier
                        .padding(horizontal = 4.dp)
                        .size(10.dp),
                )
            }
        }
    }
}

/** 权限缺失提示卡片 */
@Composable
private fun PermissionRequiredView(onDismiss: () -> Unit) {
    val context = LocalContext.current
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Box(
            modifier = Modifier
                .size(64.dp)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.errorContainer),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_folder),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onErrorContainer,
                modifier = Modifier.size(32.dp),
            )
        }
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = stringResource(R.string.folder_picker_title),
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.Bold,
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = stringResource(R.string.folder_picker_perm_hint),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 16.dp),
        )
        Spacer(modifier = Modifier.height(24.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.common_cancel))
            }
            Button(
                onClick = {
                    if (Build.VERSION.SDK_INT >= 30) {
                        try {
                            val intent = Intent(
                                Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                                Uri.parse("package:${context.packageName}"),
                            )
                            context.startActivity(intent)
                        } catch (_: Exception) {
                            context.startActivity(Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION))
                        }
                    }
                    onDismiss()
                },
            ) {
                Text(stringResource(R.string.folder_picker_perm_btn))
            }
        }
    }
}
