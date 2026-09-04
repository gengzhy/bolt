package com.lt.transfer.engine

import android.content.Context
import android.net.Uri
import androidx.documentfile.provider.DocumentFile
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileOutputStream
import java.util.UUID

/**
 * SAF → 私有目录暂存复制。
 *
 * 背景：Rust 核心经 std::fs 读取真实文件路径，无法打开 `content://` URI；
 * SAF 的临时读授权只在进程存活期有效。因此选中后先把文件（或整棵目录树）
 * 复制到 `filesDir/outbox/<nonce>/`，再把暂存后的绝对路径交给
 * `lt_send_files`。断点续传（lt_resume_task 会重新遍历 source_paths）
 * 也因此可行——暂存目录在任务终态前一直保留。
 *
 * 代价：空间占用翻倍 + 一次本地复制耗时（同分区拷贝），换取无需
 * MANAGE_EXTERNAL_STORAGE 即可发送任意 SAF 位置的文件。
 */
object SendStager {

    /** 暂存结果：根目录（清理用）+ 传给 lt_send_files 的路径数组。 */
    data class Staged(val rootDir: File, val paths: List<String>, val totalBytes: Long)

    /** OpenMultipleDocuments 结果：多文件平铺暂存。 */
    suspend fun stageFiles(context: Context, uris: List<Uri>): Staged? =
        withContext(Dispatchers.IO) {
            if (uris.isEmpty()) return@withContext null
            val root = outboxRoot(context)
            var total = 0L
            val paths = ArrayList<String>(uris.size)
            try {
                for (uri in uris) {
                    val doc = DocumentFile.fromSingleUri(context, uri) ?: continue
                    val name = sanitize(doc.name ?: "file")
                    val dest = uniqueFile(root, name)
                    total += copyInto(context, uri, dest)
                    paths += dest.absolutePath
                }
            } catch (e: Exception) {
                root.deleteRecursively()
                throw e
            }
            if (paths.isEmpty()) {
                root.deleteRecursively()
                null
            } else {
                Staged(root, paths, total)
            }
        }

    /** OpenDocumentTree 结果：整棵目录树按相对结构暂存。 */
    suspend fun stageTree(context: Context, treeUri: Uri): Staged? =
        withContext(Dispatchers.IO) {
            val tree = DocumentFile.fromTreeUri(context, treeUri) ?: return@withContext null
            val root = outboxRoot(context)
            try {
                val total = copyTree(context, tree, root)
                val topLevel = root.listFiles().orEmpty()
                if (topLevel.isEmpty() || total == 0L) {
                    root.deleteRecursively()
                    null
                } else {
                    // 目录整体交给 Rust 遍历（lt_send_files 支持目录项）
                    Staged(root, topLevel.map { it.absolutePath }, total)
                }
            } catch (e: Exception) {
                root.deleteRecursively()
                throw e
            }
        }

    /** 发送失败等场景的手动清理。 */
    fun discard(staged: Staged?) {
        staged?.rootDir?.deleteRecursively()
    }

    // ---------------- 内部 ----------------

    private fun outboxRoot(context: Context): File {
        val dir = File(File(context.filesDir, "outbox"), UUID.randomUUID().toString())
        dir.mkdirs()
        return dir
    }

    private fun copyTree(context: Context, doc: DocumentFile, destDir: File): Long {
        var total = 0L
        for (child in doc.listFiles()) {
            val name = sanitize(child.name ?: continue)
            when {
                child.isDirectory -> {
                    val sub = File(destDir, name)
                    sub.mkdirs()
                    total += copyTree(context, child, sub)
                }
                child.isFile -> {
                    total += copyInto(context, child.uri, uniqueFile(destDir, name))
                }
                // 符号链接等非常规项跳过（与 Rust 遍历层行为一致）
            }
        }
        return total
    }

    private fun copyInto(context: Context, uri: Uri, dest: File): Long {
        val input = context.contentResolver.openInputStream(uri)
            ?: throw java.io.IOException("无法打开 $uri")
        input.use { ins ->
            FileOutputStream(dest).use { out ->
                val buf = ByteArray(256 * 1024)
                while (true) {
                    val n = ins.read(buf)
                    if (n <= 0) break
                    out.write(buf, 0, n)
                }
                out.flush()
            }
        }
        return dest.length()
    }

    /** 去掉路径分隔符等危险字符。 */
    private fun sanitize(name: String): String =
        name.replace('/', '_').replace('\\', '_').ifEmpty { "file" }

    /** 同名文件自动加序号，避免覆盖。 */
    private fun uniqueFile(dir: File, name: String): File {
        var candidate = File(dir, name)
        var i = 1
        val dot = name.lastIndexOf('.')
        val stem = if (dot > 0) name.substring(0, dot) else name
        val ext = if (dot > 0) name.substring(dot) else ""
        while (candidate.exists()) {
            candidate = File(dir, "$stem($i)$ext")
            i++
        }
        return candidate
    }
}
