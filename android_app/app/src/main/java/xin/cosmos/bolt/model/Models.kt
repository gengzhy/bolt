package xin.cosmos.bolt.model

/**
 * UI 状态模型（需求分析报告 §2.6：UI 层只做渲染，业务逻辑全在 Rust 核心）。
 *
 * 数据来源只有两处：
 * 1. Rust 事件回调（EVT_*，见 docs/ffi_api.md）；
 * 2. `bt_get_tasks` / `bt_get_devices` / `bt_get_config` 的 JSON 快照。
 */

/** 设备连接状态（由 EVT_CONN_STATE 驱动，合并进设备行）。 */
enum class ConnectionState { None, Connecting, Connected, Disconnected }

/** 局域网设备（bt_get_devices / EVT_DEVICE_LIST 数组元素）。 */
data class DeviceUi(
    val uuid: String,
    val name: String,
    val ip: String,
    val quicPort: Int,
    val tcpPort: Int,
    val deviceType: Int,
    val stealth: Boolean = false,
    val source: String = "",
    val connState: ConnectionState = ConnectionState.None,
    val transport: String = "",
)

/** 任务状态字符串（与 Rust TaskState::as_str 一致）。 */
object TaskStates {
    const val WAITING_ACCEPT = "waiting_accept"
    const val TRANSFERRING = "transferring"
    const val PAUSED = "paused"
    const val DONE = "done"
    const val CANCELLED = "cancelled"
    const val ERROR = "error"
    const val REJECTED = "rejected"

    fun isActive(state: String): Boolean =
        state == WAITING_ACCEPT || state == TRANSFERRING

    fun isTerminal(state: String): Boolean =
        state == DONE || state == CANCELLED || state == ERROR || state == REJECTED
}

/** 传输任务（bt_get_tasks 数组元素 + EVT_TASK_PROGRESS 增量）。 */
data class TaskUi(
    val taskId: Long,
    val incoming: Boolean,
    val peerUuid: String = "",
    val peerName: String = "",
    val fileCount: Int = 0,
    val totalSize: Long = 0,
    val doneBytes: Long = 0,
    val okFiles: Int = 0,
    val failedFiles: Int = 0,
    val state: String = TaskStates.WAITING_ACCEPT,
    val currentFile: String = "",
    /** 任务实际使用的传输协议（quic/tcp），会话建立后由后端回填 */
    val transport: String = "",
    val rateBps: Long = 0,
    val etaSecs: Long = 0,
    val avgRateBps: Long = 0,
    val durationMs: Long = 0,
    val startTimeMs: Long = 0,
    val createdUnix: Long = 0,
)

/** 传输记录日志（持久化保存的历史传输项）。 */
data class TransferLogUi(
    val id: String,
    val taskId: Long,
    val incoming: Boolean,
    val transport: String = "",
    val peerName: String = "",
    val peerUuid: String = "",
    val fileName: String = "",
    val fileCount: Int = 1,
    val totalSize: Long = 0,
    val avgRateBps: Long = 0,
    val startTimeMs: Long = 0,
    val endTimeMs: Long = 0,
    val durationMs: Long = 0,
    val state: String = TaskStates.DONE,
    val okFiles: Int = 0,
    val failedFiles: Int = 0,
)

/** 待用户响应的弹窗（队列，逐个展示）。 */
sealed class PendingDialog {
    /** 配对请求（EVT_PAIR_REQUEST）：展示验证码让用户跨屏比对。 */
    data class PairRequest(
        val pairId: Long,
        val uuid: String,
        val name: String,
        val code: String,
        val isInitiator: Boolean = false,
    ) : PendingDialog()

    /** 入站传输请求（EVT_TRANSFER_REQUEST）。 */
    data class TransferRequest(
        val reqId: Long,
        val uuid: String,
        val name: String,
        val fileCount: Int,
        val totalSize: Long,
    ) : PendingDialog()
}

/** 一次性提示（Snackbar），不进状态树。 */
sealed class OneShotEvent {
    data class Info(val message: String) : OneShotEvent()
    data class Error(val code: Int, val message: String) : OneShotEvent()
}

/** 设置面板（bt_get_config 全量 JSON 的 UI 视图）。 */
data class ConfigUi(
    val deviceName: String = "",
    val saveDir: String = "",
    val stealthMode: Boolean = false,
    val autoAcceptTrusted: Boolean = false,
    val concurrency: Int = 0,
    val listenPort: Int = 0,
    val chunkSize: Long = 0,
    val preferQuic: Boolean = true,
    val useMdns: Boolean = true,
    val collision: String = "rename",
)

/** 顶层 UI 状态。 */
data class UiState(
    val engineReady: Boolean = false,
    val initError: Int = 0,
    val fingerprint: String = "",
    val version: String = "",
    val devices: List<DeviceUi> = emptyList(),
    /** 设备扫描进行中（点「刷新」触发；收到非空设备列表或超时后结束） */
    val scanning: Boolean = false,
    val tasks: Map<Long, TaskUi> = emptyMap(),
    val pendingDialogs: List<PendingDialog> = emptyList(),
    val config: ConfigUi = ConfigUi(),
    /** 本机全部非环回 IPv4（设置页展示，来自 bt_get_local_info） */
    val localIps: List<String> = emptyList(),
    /** 实际监听端口（端口池避让后的真实值） */
    val localPort: Int = 0,
    /** SAF 暂存（复制选中文件到私有目录）进行中 */
    val staging: Boolean = false,
)
