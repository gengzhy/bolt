package xin.cosmos.bolt.engine

import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.OpenableColumns
import androidx.documentfile.provider.DocumentFile
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import xin.cosmos.bolt.R

/**
 * 外部系统分享数据载荷：
 * 包含解析后的 Uri 列表以及简要的文件摘要文本。
 */
data class SharePayload(
    val uris: List<Uri>,
    val summaryText: String,
    val firstFileName: String,
)

/**
 * 系统分享（ACTION_SEND / ACTION_SEND_MULTIPLE）解析与全局状态管理助手。
 */
object SharePayloadHelper {

    private val _pendingShare = MutableStateFlow<SharePayload?>(null)
    val pendingShare = _pendingShare.asStateFlow()

    /**
     * 解析传入的 Intent。如果是分享事件，则提取其中的所有文件 Uri 并激活全局待发送状态。
     * @return 如果成功捕获并解析了有效分享数据则返回 true，否则返回 false。
     */
    fun handleShareIntent(context: Context, intent: Intent?): Boolean {
        if (intent == null) return false
        val action = intent.action ?: return false

        if (action != Intent.ACTION_SEND && action != Intent.ACTION_SEND_MULTIPLE) {
            return false
        }

        val uriList = ArrayList<Uri>()

        // 1. 优先从 EXTRA_STREAM 读取
        if (action == Intent.ACTION_SEND) {
            val streamUri = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
            } else {
                @Suppress("DEPRECATION")
                intent.getParcelableExtra(Intent.EXTRA_STREAM)
            }
            if (streamUri != null) {
                uriList.add(streamUri)
            }
        } else {
            val streamList = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
            } else {
                @Suppress("DEPRECATION")
                intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM)
            }
            if (!streamList.isNullOrEmpty()) {
                uriList.addAll(streamList.filterNotNull())
            }
        }

        // 2. 兜底检查 ClipData（某些应用或厂商分享时将文件放在 ClipData 中）
        intent.clipData?.let { clipData ->
            for (i in 0 until clipData.itemCount) {
                val item = clipData.getItemAt(i)
                val itemUri = item.uri
                if (itemUri != null && !uriList.contains(itemUri)) {
                    uriList.add(itemUri)
                }
            }
        }

        // 3. 兜底检查 intent.data
        intent.data?.let { dataUri ->
            if (!uriList.contains(dataUri)) {
                uriList.add(dataUri)
            }
        }

        if (uriList.isEmpty()) {
            return false
        }

        // 提取第一个文件名做概要展示
        val firstUri = uriList.first()
        val firstName = resolveDisplayName(context, firstUri)
        val summary = if (uriList.size == 1) {
            context.getString(R.string.quick_share_file_summary_single, firstName)
        } else {
            context.getString(R.string.quick_share_file_summary, uriList.size, firstName)
        }

        _pendingShare.value = SharePayload(
            uris = uriList,
            summaryText = summary,
            firstFileName = firstName,
        )
        return true
    }

    /** 清理当前的分享载荷（用户已发送或主动取消）。 */
    fun clear() {
        _pendingShare.value = null
    }

    /** 解析 Uri 对应的真实文件展示名。 */
    fun resolveDisplayName(context: Context, uri: Uri): String {
        // 1. 尝试使用 DocumentFile
        try {
            val doc = DocumentFile.fromSingleUri(context, uri)
            val name = doc?.name
            if (!name.isNullOrBlank()) return name
        } catch (_: Exception) {}

        // 2. 尝试 ContentResolver 查询 DISPLAY_NAME
        if (uri.scheme == "content") {
            try {
                context.contentResolver.query(
                    uri,
                    arrayOf(OpenableColumns.DISPLAY_NAME),
                    null,
                    null,
                    null,
                )?.use { cursor ->
                    val idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (idx != -1 && cursor.moveToFirst()) {
                        val name = cursor.getString(idx)
                        if (!name.isNullOrBlank()) return name
                    }
                }
            } catch (_: Exception) {}
        }

        // 3. 尝试读取路径段
        val seg = uri.lastPathSegment?.substringAfterLast('/')
        if (!seg.isNullOrBlank()) return seg

        return "file"
    }
}
