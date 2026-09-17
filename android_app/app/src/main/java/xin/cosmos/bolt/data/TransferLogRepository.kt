package xin.cosmos.bolt.data

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONArray
import org.json.JSONObject
import xin.cosmos.bolt.model.TaskStates
import xin.cosmos.bolt.model.TaskUi
import xin.cosmos.bolt.model.TransferLogUi
import java.io.File

/**
 * 传输历史记录日志持久化仓储（存放在应用专属内部文件目录 transfer_logs.json 中）。
 * 即使任务在主界面被「清除记录」移除，此处的审计与追溯日志仍完好保存。
 */
object TransferLogRepository {
    private const val FILE_NAME = "transfer_logs.json"
    private const val PREFS_NAME = "bolt_transfer_log_prefs"
    private const val KEY_MAX_LOGS_COUNT = "max_logs_count"
    const val DEFAULT_MAX_LOGS_COUNT = 40000

    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())
    private val mutex = Mutex()
    private var logFile: File? = null

    private val _logs = MutableStateFlow<List<TransferLogUi>>(emptyList())
    val logs: StateFlow<List<TransferLogUi>> = _logs.asStateFlow()

    private val _maxLogsCount = MutableStateFlow(DEFAULT_MAX_LOGS_COUNT)
    val maxLogsCount: StateFlow<Int> = _maxLogsCount.asStateFlow()

    fun init(context: Context) {
        try {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            val saved = prefs.getInt(KEY_MAX_LOGS_COUNT, DEFAULT_MAX_LOGS_COUNT)
            _maxLogsCount.value = if (saved > 0) saved else DEFAULT_MAX_LOGS_COUNT
        } catch (e: Exception) {
            e.printStackTrace()
        }

        if (logFile != null) return
        logFile = File(context.filesDir, FILE_NAME)
        scope.launch {
            loadFromDisk()
        }
    }

    fun setMaxLogsCount(context: Context, count: Int) {
        val valid = if (count > 0) count else DEFAULT_MAX_LOGS_COUNT
        _maxLogsCount.value = valid
        try {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            prefs.edit().putInt(KEY_MAX_LOGS_COUNT, valid).apply()
        } catch (e: Exception) {
            e.printStackTrace()
        }
        scope.launch {
            mutex.withLock {
                val current = _logs.value
                if (current.size > valid) {
                    val capped = current.take(valid)
                    _logs.value = capped
                    saveToDiskLocked(capped)
                }
            }
        }
    }

    private suspend fun loadFromDisk() = mutex.withLock {
        val file = logFile ?: return@withLock
        if (!file.exists()) {
            _logs.value = emptyList()
            return@withLock
        }
        try {
            val content = file.readText()
            val array = JSONArray(content)
            val list = ArrayList<TransferLogUi>(array.length())
            for (i in 0 until array.length()) {
                val o = array.optJSONObject(i) ?: continue
                list.add(
                    TransferLogUi(
                        id = o.optString("id"),
                        taskId = o.optLong("task_id"),
                        incoming = o.optBoolean("incoming"),
                        transport = o.optString("transport"),
                        peerName = o.optString("peer_name"),
                        peerUuid = o.optString("peer_uuid"),
                        fileName = o.optString("file_name"),
                        fileCount = o.optInt("file_count", 1),
                        totalSize = o.optLong("total_size"),
                        avgRateBps = o.optLong("avg_rate_bps"),
                        startTimeMs = o.optLong("start_time_ms"),
                        endTimeMs = o.optLong("end_time_ms"),
                        durationMs = o.optLong("duration_ms"),
                        state = o.optString("state", TaskStates.DONE),
                        okFiles = o.optInt("ok_files"),
                        failedFiles = o.optInt("failed_files"),
                    )
                )
            }
            // 保证按开始时间倒序
            list.sortByDescending { if (it.startTimeMs > 0) it.startTimeMs else it.taskId }
            _logs.value = list
        } catch (e: Exception) {
            e.printStackTrace()
            _logs.value = emptyList()
        }
    }

    fun record(task: TaskUi) {
        scope.launch {
            mutex.withLock {
                val current = _logs.value.toMutableList()
                val logId = if (task.createdUnix > 0) {
                    "task_${task.taskId}_${task.createdUnix}"
                } else {
                    "task_${task.taskId}"
                }

                val startMs = when {
                    task.startTimeMs > 0 -> task.startTimeMs
                    task.createdUnix > 0 -> task.createdUnix * 1000
                    else -> System.currentTimeMillis()
                }

                val endMs = when {
                    task.durationMs > 0 -> startMs + task.durationMs
                    TaskStates.isTerminal(task.state) -> System.currentTimeMillis()
                    else -> 0L
                }

                val fileName = when {
                    task.currentFile.isNotBlank() -> task.currentFile
                    task.fileCount > 1 -> "${task.fileCount} 个文件"
                    task.peerName.isNotBlank() -> "来自 ${task.peerName} 的文件"
                    else -> "传输任务 #${task.taskId}"
                }

                val rate = when {
                    task.avgRateBps > 0 -> task.avgRateBps
                    task.durationMs > 0 && task.doneBytes > 0 -> (task.doneBytes * 1000 / task.durationMs)
                    else -> task.rateBps
                }

                val newEntry = TransferLogUi(
                    id = logId,
                    taskId = task.taskId,
                    incoming = task.incoming,
                    transport = task.transport,
                    peerName = task.peerName,
                    peerUuid = task.peerUuid,
                    fileName = fileName,
                    fileCount = if (task.fileCount > 0) task.fileCount else 1,
                    totalSize = if (task.totalSize > 0) task.totalSize else task.doneBytes,
                    avgRateBps = rate,
                    startTimeMs = startMs,
                    endTimeMs = endMs,
                    durationMs = task.durationMs,
                    state = task.state,
                    okFiles = task.okFiles,
                    failedFiles = task.failedFiles,
                )

                val existingIndex = current.indexOfFirst { it.taskId == task.taskId }
                if (existingIndex >= 0) {
                    // 保留较早的合法 startMs
                    val old = current[existingIndex]
                    val preservedStart = if (old.startTimeMs in 1 until startMs) old.startTimeMs else startMs
                    current[existingIndex] = newEntry.copy(startTimeMs = preservedStart)
                } else {
                    current.add(0, newEntry)
                }

                current.sortByDescending { if (it.startTimeMs > 0) it.startTimeMs else it.taskId }

                val limit = _maxLogsCount.value
                val capped = if (current.size > limit) current.take(limit) else current
                _logs.value = capped
                saveToDiskLocked(capped)
            }
        }
    }

    fun clearLogs() {
        scope.launch {
            mutex.withLock {
                _logs.value = emptyList()
                saveToDiskLocked(emptyList())
            }
        }
    }

    private fun saveToDiskLocked(list: List<TransferLogUi>) {
        val file = logFile ?: return
        try {
            val array = JSONArray()
            for (item in list) {
                val o = JSONObject()
                o.put("id", item.id)
                o.put("task_id", item.taskId)
                o.put("incoming", item.incoming)
                o.put("transport", item.transport)
                o.put("peer_name", item.peerName)
                o.put("peer_uuid", item.peerUuid)
                o.put("file_name", item.fileName)
                o.put("file_count", item.fileCount)
                o.put("total_size", item.totalSize)
                o.put("avg_rate_bps", item.avgRateBps)
                o.put("start_time_ms", item.startTimeMs)
                o.put("end_time_ms", item.endTimeMs)
                o.put("duration_ms", item.durationMs)
                o.put("state", item.state)
                o.put("ok_files", item.okFiles)
                o.put("failed_files", item.failedFiles)
                array.put(o)
            }
            file.writeText(array.toString())
        } catch (e: Exception) {
            e.printStackTrace()
        }
    }
}
