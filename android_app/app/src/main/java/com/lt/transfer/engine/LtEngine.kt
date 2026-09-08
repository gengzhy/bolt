package com.lt.transfer.engine

import android.content.Context
import android.content.Intent
import android.media.MediaScannerConnection
import android.os.Environment
import android.util.Log
import androidx.core.content.ContextCompat
import com.lt.transfer.TransferService
import com.lt.transfer.ffi.Native
import com.lt.transfer.model.ConfigUi
import com.lt.transfer.model.ConnectionState
import com.lt.transfer.model.DeviceUi
import com.lt.transfer.model.OneShotEvent
import com.lt.transfer.model.PendingDialog
import com.lt.transfer.model.TaskStates
import com.lt.transfer.model.TaskUi
import com.lt.transfer.model.UiState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * Rust 核心的进程级门面（单例）。
 *
 * 生命周期设计（需求 §2.6 前台服务保活的前提）：
 * - 由 [com.lt.transfer.LtApplication.onCreate] 初始化，进程存活期间常驻；
 * - **绝不调用 lt_shutdown**——Activity 销毁不能中断传输，锁屏后前台服务
 *   （[TransferService]）继续持有通知，Rust 引擎在本进程内继续收发；
 * - 事件回调在 Rust 专用线程执行，这里只做入队（回调内禁止重入 lt_*），
 *   由主线程协程统一消费并更新 [uiState]。
 */
object LtEngine {

    private const val TAG = "LtEngine"

    /** 刷新扫描的可视化窗口：进度条最长亮这么久，收到非空设备列表提前熄灭。 */
    private const val SCAN_WINDOW_MS = 5000L

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val rawEvents = Channel<Pair<Int, String>>(Channel.UNLIMITED)

    private val _uiState = MutableStateFlow(UiState())
    val uiState: StateFlow<UiState> = _uiState

    private val _oneShots = MutableSharedFlow<OneShotEvent>(extraBufferCapacity = 64)
    val oneShots: SharedFlow<OneShotEvent> = _oneShots

    /** 「刷新」扫描进度条的自动熄灭任务（重复点击时重置）。 */
    private var scanStopJob: Job? = null

    private lateinit var appContext: Context
    private var initialized = false

    /** 发送任务的暂存目录（SAF → 私有目录副本），任务终态时清理。 */
    private val outboxByTask = HashMap<Long, File>()

    @Synchronized
    fun init(context: Context) {
        if (initialized) return
        initialized = true
        appContext = context.applicationContext

        // 数据目录放 filesDir/lt：FileProvider（files-path）可覆盖其下接收文件
        val dataDir = File(appContext.filesDir, "lt")
        val code = Native.ltInit(dataDir.absolutePath)
        if (code != 0) {
            Log.e(TAG, "lt_init 失败：$code")
            _uiState.value = _uiState.value.copy(initError = code)
            return
        }
        Native.ltSetEventCallback { eventId, payload ->
            rawEvents.trySend(eventId to (payload ?: ""))
        }

        // 单消费者：所有状态变更都在主线程，天然无竞态
        scope.launch {
            for ((id, payload) in rawEvents) handleEvent(id, payload)
        }
        // 传输中每 2s 用 lt_get_tasks 全量对账（补 current_file 等事件未覆盖字段）
        scope.launch {
            while (true) {
                delay(2000)
                if (_uiState.value.tasks.values.any { TaskStates.isActive(it.state) }) {
                    syncTasks()
                }
            }
        }

        ensureDefaultDeviceName()
        refreshStaticInfo()
        syncTasks()
        Native.ltStartDiscovery()
        NsdHelper.start(appContext)
        _uiState.value = _uiState.value.copy(engineReady = true)
    }

    fun dataDir(): File = File(appContext.filesDir, "lt")

    // ---------------- 静态信息 / 对账 ----------------

    /**
     * 设备名取「设置→关于手机」中的设备名称（Settings.Global.DEVICE_NAME，
     * 读不到时回落设备型号）。仅当前名称仍是历版默认值时才改写，
     * 用户自定义名称不受影响。必须在 ltStartDiscovery / NsdHelper.start
     * 之前，广播名才取到新值。
     */
    private fun ensureDefaultDeviceName() {
        val current = parseConfig(Native.ltGetConfig()).deviceName
        val model = android.os.Build.MODEL.trim()
        val isDefault = current.isEmpty() ||
            current == "device" ||
            current == model ||
            Regex("^[A-Za-z0-9._-]+-[0-9a-f]{4}$").matches(current)
        if (!isDefault) return
        val aboutName = try {
            android.provider.Settings.Global.getString(
                appContext.contentResolver,
                android.provider.Settings.Global.DEVICE_NAME,
            )
        } catch (_: Exception) {
            null
        }?.trim().orEmpty()
        val target = aboutName.ifEmpty { model }
        if (target.isEmpty() || target == "device" || target == current) return
        Native.ltSetConfig(JSONObject().put("device_name", target).toString())
    }

    private fun refreshStaticInfo() {
        val cfgJson = Native.ltGetConfig()
        val info = try {
            JSONObject(Native.ltGetLocalInfo())
        } catch (_: Exception) {
            null
        }
        val ips = mutableListOf<String>()
        info?.optJSONArray("ips")?.let { arr ->
            for (i in 0 until arr.length()) ips.add(arr.optString(i))
        }
        _uiState.value = _uiState.value.copy(
            fingerprint = Native.ltGetLocalFingerprint(),
            version = Native.ltVersion(),
            config = parseConfig(cfgJson),
            localIps = ips,
            localPort = info?.optInt("qport", 0) ?: 0,
        )
    }

    fun syncTasks() {
        val arr = try {
            JSONArray(Native.ltGetTasks())
        } catch (_: Exception) {
            return
        }
        val map = HashMap<Long, TaskUi>()
        for (i in 0 until arr.length()) {
            val t = parseTask(arr.getJSONObject(i)) ?: continue
            // 保留正在传输中事件已推进的进度；若任务已暂停或终态，则以引擎确定的快照为准对齐
            val old = _uiState.value.tasks[t.taskId]
            map[t.taskId] = if (old != null && old.doneBytes > t.doneBytes &&
                old.state == t.state && old.state != TaskStates.PAUSED
            ) old else t
        }
        updateState { it.copy(tasks = map) }
        syncServiceWithActiveTasks()
    }

    private fun refreshDevices() {
        val arr = try {
            JSONArray(Native.ltGetDevices())
        } catch (_: Exception) {
            return
        }
        val old = _uiState.value.devices.associateBy { it.uuid }
        val list = ArrayList<DeviceUi>(arr.length())
        for (i in 0 until arr.length()) {
            val o = arr.getJSONObject(i)
            val uuid = o.optString("uuid")
            list.add(
                DeviceUi(
                    uuid = uuid,
                    name = o.optString("name", uuid),
                    ip = o.optString("ip"),
                    quicPort = o.optInt("quic_port"),
                    tcpPort = o.optInt("tcp_port"),
                    deviceType = o.optInt("device_type"),
                    stealth = o.optBoolean("stealth"),
                    source = o.optString("source"),
                    connState = old[uuid]?.connState ?: ConnectionState.None,
                    transport = old[uuid]?.transport ?: "",
                ),
            )
        }
        updateState { it.copy(devices = list) }
        if (list.isNotEmpty()) stopScanIndicator()
        Log.i(TAG, "设备列表刷新：${list.size} 台")
    }

    fun refreshConfig() {
        _uiState.value = _uiState.value.copy(config = parseConfig(Native.ltGetConfig()))
    }

    // ---------------- 事件处理（主线程） ----------------

    private fun handleEvent(eventId: Int, payload: String) {
        when (eventId) {
            Native.EVT_DEVICE_LIST -> refreshDevices()

            Native.EVT_CONN_STATE -> {
                val p = try { JSONObject(payload) } catch (_: Exception) { return }
                val uuid = p.optString("uuid")
                val connected = p.optString("state") == "connected"
                val transport = p.optString("transport")
                updateState { s ->
                    s.copy(
                        devices = s.devices.map {
                            if (it.uuid == uuid) {
                                it.copy(
                                    connState = if (connected) {
                                        ConnectionState.Connected
                                    } else {
                                        ConnectionState.Disconnected
                                    },
                                    transport = if (connected) transport else "",
                                )
                            } else it
                        },
                    )
                }
                val err = p.optInt("err", 0)
                if (!connected && err != 0) {
                    postOneShot(OneShotEvent.Error(err, "与 ${p.optString("name", uuid)} 的连接已断开"))
                }
            }

            Native.EVT_PAIR_REQUEST -> {
                val p = try { JSONObject(payload) } catch (_: Exception) { return }
                pushDialog(
                    PendingDialog.PairRequest(
                        pairId = p.optLong("pair_id"),
                        uuid = p.optString("uuid"),
                        name = p.optString("name"),
                        code = p.optString("code"),
                    ),
                )
            }

            Native.EVT_TRANSFER_REQUEST -> {
                val p = try { JSONObject(payload) } catch (_: Exception) { return }
                pushDialog(
                    PendingDialog.TransferRequest(
                        reqId = p.optLong("req_id"),
                        uuid = p.optString("uuid"),
                        name = p.optString("name"),
                        fileCount = p.optInt("file_count"),
                        totalSize = p.optLong("total_size"),
                    ),
                )
            }

            Native.EVT_TASK_STATE -> {
                val p = try { JSONObject(payload) } catch (_: Exception) { return }
                val taskId = p.optLong("task_id")
                val state = p.optString("state")
                val incoming = p.optBoolean("incoming")
                updateState { s ->
                    val t = s.tasks[taskId]
                        ?: TaskUi(taskId = taskId, incoming = incoming)
                    s.copy(tasks = s.tasks + (taskId to t.copy(
                        state = state,
                        rateBps = if (state == TaskStates.PAUSED) 0 else t.rateBps,
                        etaSecs = if (state == TaskStates.PAUSED) 0 else t.etaSecs,
                    )))
                }
                if (state == TaskStates.REJECTED) {
                    postOneShot(OneShotEvent.Info("对方拒绝了本次传输"))
                }
                if (state == TaskStates.DONE || state == TaskStates.CANCELLED) {
                    cleanupOutbox(taskId)
                }
                syncServiceWithActiveTasks()
            }

            Native.EVT_TASK_PROGRESS -> {
                val p = try { JSONObject(payload) } catch (_: Exception) { return }
                val taskId = p.optLong("task_id")
                updateState { s ->
                    val t = s.tasks[taskId] ?: TaskUi(
                        taskId = taskId,
                        incoming = p.optBoolean("incoming"),
                        state = TaskStates.TRANSFERRING,
                    )
                    s.copy(
                        tasks = s.tasks + (taskId to t.copy(
                            doneBytes = p.optLong("done"),
                            currentFile = p.optString("rel_path", t.currentFile),
                            rateBps = if (t.state == TaskStates.PAUSED) 0 else p.optLong("rate_bps"),
                            etaSecs = if (t.state == TaskStates.PAUSED) 0 else p.optLong("eta_secs"),
                            totalSize = p.optLong("total", t.totalSize),
                        )),
                    )
                }
            }

            Native.EVT_TASK_SUMMARY -> {
                val p = try { JSONObject(payload) } catch (_: Exception) { return }
                val taskId = p.optLong("task_id")
                val ok = p.optInt("ok")
                val failed = p.optInt("failed")
                val avgRate = p.optLong("avg_rate_bps")
                val durationMs = p.optLong("duration_ms")
                val totalSize = p.optLong("total_size")
                val isSuccess = (failed == 0 && ok > 0)
                updateState { s ->
                    val t = s.tasks[taskId] ?: return@updateState s
                    val effectiveTotal = if (totalSize > 0) totalSize else t.totalSize
                    s.copy(
                        tasks = s.tasks + (taskId to t.copy(
                            okFiles = ok,
                            failedFiles = failed,
                            state = if (isSuccess) TaskStates.DONE else TaskStates.ERROR,
                            doneBytes = if (isSuccess) effectiveTotal else t.doneBytes,
                            totalSize = effectiveTotal,
                            avgRateBps = if (avgRate > 0) avgRate else t.avgRateBps,
                            durationMs = if (durationMs > 0) durationMs else t.durationMs,
                        )),
                    )
                }
                val incoming = p.optBoolean("incoming")
                val verb = if (incoming) "接收" else "发送"
                postOneShot(OneShotEvent.Info("${verb}完成：成功 $ok 个，失败 $failed 个"))
                if (isSuccess) {
                    cleanupOutbox(taskId)
                }
                if (incoming && ok > 0) {
                    scanReceivedDirectory()
                }
                syncServiceWithActiveTasks()
                syncTasks()
            }

            Native.EVT_ERROR -> {
                val p = try { JSONObject(payload) } catch (_: Exception) { return }
                val code = p.optInt("code")
                val detail = p.optString("message")
                postOneShot(
                    OneShotEvent.Error(
                        code,
                        com.lt.transfer.model.ErrorMessages.of(code, detail),
                    ),
                )
            }
        }
    }

    // ---------------- UI 操作入口（均为 FFI 透传） ----------------

    fun connect(uuid: String) {
        updateState { s ->
            s.copy(devices = s.devices.map {
                if (it.uuid == uuid && it.connState != ConnectionState.Connected) {
                    it.copy(connState = ConnectionState.Connecting)
                } else it
            })
        }
        val rc = Native.ltConnect(uuid)
        if (rc != 0) postOneShot(
            OneShotEvent.Error(rc, com.lt.transfer.model.ErrorMessages.of(rc)),
        )
    }

    fun disconnect(uuid: String) {
        Native.ltDisconnect(uuid)
    }

    fun connectAddr(ip: String, port: Int) {
        Native.ltAddManualDevice(ip, port)
        val rc = Native.ltConnectAddr(ip, port)
        if (rc != 0) postOneShot(
            OneShotEvent.Error(rc, com.lt.transfer.model.ErrorMessages.of(rc)),
        ) else postOneShot(OneShotEvent.Info("正在连接 $ip:$port …"))
    }

    fun probeNetwork() {
        Native.ltProbeNetwork()
        // Android 主发现通道是 NSD：启动时若因权限未起发现则在此重试，
        // 并立即重注入已知服务，避免列表被 10s 过期清扫清空
        NsdHelper.ensureDiscovery()
        NsdHelper.refreshNow()
        // 发现结果异步到达（UDP 探测应答 / NSD 浏览解析需数秒），
        // 期间显示扫描进度，避免点击后界面无反馈
        startScanIndicator()
    }

    /** 点亮扫描进度条；[SCAN_WINDOW_MS] 后自动熄灭，重复点击重置计时。 */
    private fun startScanIndicator() {
        updateState { it.copy(scanning = true) }
        scanStopJob?.cancel()
        scanStopJob = scope.launch {
            delay(SCAN_WINDOW_MS)
            updateState { it.copy(scanning = false) }
        }
    }

    /** 收到非空设备列表 ⇒ 扫描已有结果，立即熄灭进度条。 */
    private fun stopScanIndicator() {
        if (!_uiState.value.scanning) return
        scanStopJob?.cancel()
        updateState { it.copy(scanning = false) }
    }

    fun respondPair(pairId: Long, accept: Boolean) {
        Native.ltRespondPair(pairId, if (accept) 1 else 0)
        popDialog()
    }

    fun respondTransfer(reqId: Long, accept: Boolean) {
        Native.ltRespondTransfer(reqId, if (accept) 1 else 0)
        popDialog()
        if (accept) syncServiceWithActiveTasks()
    }

    /**
     * 发送已暂存到私有目录的文件（[SendStager] 产物）。
     * `paths` 为绝对路径数组；记录暂存目录映射，任务终态时清理。
     */
    fun sendFiles(uuid: String, paths: List<String>, stagedRoot: File) {
        val json = JSONArray(paths).toString()
        val out = LongArray(1)
        val rc = Native.ltSendFiles(uuid, json, out)
        if (rc != 0) {
            stagedRoot.deleteRecursively()
            postOneShot(
                OneShotEvent.Error(rc, com.lt.transfer.model.ErrorMessages.of(rc)),
            )
            return
        }
        outboxByTask[out[0]] = stagedRoot
        syncTasks()
        syncServiceWithActiveTasks()
    }

    fun cancelTask(taskId: Long) {
        Native.ltCancelTask(taskId)
    }

    fun setConfig(patch: JSONObject) {
        val rc = Native.ltSetConfig(patch.toString())
        if (rc != 0) {
            postOneShot(OneShotEvent.Error(rc, "设置保存失败"))
            return
        }
        refreshConfig()
        // 会话/引擎参数已由 Rust 核心热更（对此后新建的连接生效），
        // 此处只处理 NSD 侧的副作用；同时刷新本机静态信息（实际监听端口）
        refreshStaticInfo()
        // 隐身模式变化需重新注册/注销 NSD 广播
        if (patch.has("stealth_mode")) NsdHelper.applyStealth()
        // 监听端口变更：引擎空闲即重启（在途任务结束后补执行），按新端口重注册 NSD
        if (patch.has("listen_port")) NsdHelper.reregister()
        // 传输协议变更：NSD 服务的 ptcp 属性（对端据此取协议并集）需重注册
        if (patch.has("prefer_quic")) NsdHelper.reregister()
        // mDNS/NSD 发现开关
        if (patch.has("use_mdns")) {
            NsdHelper.applyEnabled(patch.optBoolean("use_mdns", true))
        }
    }

    fun clearRecords() {
        Native.ltClearRecords()
        outboxByTask.values.forEach { it.deleteRecursively() }
        outboxByTask.clear()
        syncTasks()
        postOneShot(OneShotEvent.Info("已清除传输记录"))
    }

    fun clearTempCache() {
        val rc = Native.ltClearTempCache()
        postOneShot(
            if (rc == 0) OneShotEvent.Info("已清理临时缓存")
            else OneShotEvent.Error(rc, com.lt.transfer.model.ErrorMessages.of(rc)),
        )
    }

    fun setStaging(active: Boolean) {
        updateState { it.copy(staging = active) }
    }

    // ---------------- 内部 ----------------

    private inline fun updateState(crossinline f: (UiState) -> UiState) {
        _uiState.value = f(_uiState.value)
    }

    private fun pushDialog(dialog: PendingDialog) {
        updateState { it.copy(pendingDialogs = it.pendingDialogs + dialog) }
    }

    private fun popDialog() {
        updateState {
            it.copy(pendingDialogs = it.pendingDialogs.drop(1))
        }
    }

    private fun postOneShot(event: OneShotEvent) {
        _oneShots.tryEmit(event)
    }

    private fun cleanupOutbox(taskId: Long) {
        outboxByTask.remove(taskId)?.deleteRecursively()
    }

    private fun syncServiceWithActiveTasks() {
        val anyActive = _uiState.value.tasks.values.any { TaskStates.isActive(it.state) }
        if (!anyActive) return // 服务自己在无活跃任务时退出
        try {
            ContextCompat.startForegroundService(
                appContext,
                Intent(appContext, TransferService::class.java),
            )
        } catch (e: Exception) {
            // API 31+ 后台启动受限等场景：传输继续，仅失去前台通知
            Log.w(TAG, "前台服务启动失败：$e")
        }
    }

    private fun parseConfig(json: String): ConfigUi = try {
        val o = JSONObject(json)
        ConfigUi(
            deviceName = o.optString("device_name"),
            saveDir = o.optString("save_dir"),
            stealthMode = o.optBoolean("stealth_mode"),
            autoAcceptTrusted = o.optBoolean("auto_accept_trusted"),
            concurrency = o.optInt("concurrency"),
            listenPort = o.optInt("listen_port"),
            chunkSize = o.optLong("chunk_size"),
            preferQuic = o.optBoolean("prefer_quic", true),
            useMdns = o.optBoolean("use_mdns", true),
            collision = o.optString("collision", "rename"),
        )
    } catch (_: Exception) {
        ConfigUi()
    }

    private fun parseTask(o: JSONObject): TaskUi? {
        val id = o.optLong("task_id")
        if (id == 0L) return null
        return TaskUi(
            taskId = id,
            incoming = o.optString("direction") == "recv",
            peerUuid = o.optString("peer_uuid"),
            peerName = o.optString("peer_name"),
            fileCount = o.optInt("file_count"),
            totalSize = o.optLong("total_size"),
            doneBytes = o.optLong("done_bytes"),
            okFiles = o.optInt("ok_files"),
            failedFiles = o.optInt("failed_files"),
            state = o.optString("state", TaskStates.WAITING_ACCEPT),
            currentFile = o.optString("current_file"),
            transport = o.optString("transport"),
            rateBps = o.optLong("rate_bps"),
            etaSecs = o.optLong("eta_secs"),
            avgRateBps = o.optLong("avg_rate_bps"),
            durationMs = o.optLong("duration_ms"),
        )
    }

    private fun scanReceivedDirectory() {
        try {
            val saveDir = _uiState.value.config.saveDir
            val defaultRoot = File(Environment.getExternalStorageDirectory(), "Download/LocalTransfer")
            val root = if (saveDir.isNotEmpty()) File(saveDir) else defaultRoot
            if (root.exists()) {
                val filePaths = root.walkTopDown()
                    .maxDepth(3)
                    .filter { it.isFile }
                    .map { it.absolutePath }
                    .toList()
                if (filePaths.isNotEmpty()) {
                    MediaScannerConnection.scanFile(
                        appContext,
                        filePaths.toTypedArray(),
                        null,
                        null,
                    )
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "scanReceivedDirectory 失败: $e")
        }
    }
}
