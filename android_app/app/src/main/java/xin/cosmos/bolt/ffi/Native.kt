package xin.cosmos.bolt.ffi

/**
 * 事件回调函数类型签名。
 *
 * @param eventId 事件类型 ID（对应 [Native.EVT_*] 常量，例如 [Native.EVT_DEVICE_LIST]、[Native.EVT_TASK_PROGRESS] 等）
 * @param payloadJson 事件携带的 UTF-8 JSON 格式负载数据，若事件无负载则为 null
 */
typealias BtEventCallback = (Int, String?) -> Unit

/**
 * libbt_ffi.so 的 JNI 声明与接口封装类（与 Rust 导出接口 `crates/ffi/include/bt_api.h` 一一对应）。
 *
 * 封装了 Bolt 核心引擎的生命周期管理、网络发现、设备连接、配对认证、文件收发与状态查询等 Native 接口。
 *
 * ### 线程模型与调用规范：
 * 1. **事件回调线程**：底层事件通过 Rust 专用分发线程（`bt-ffi-events`）触发。通过 JNI Attach 线程后回调传入的 [BtEventCallback]。
 *    回调方法内部仅应做轻量的数据中转（例如投递至 Kotlin 协程 Channel 或主线程 Handler），**严禁在回调线程中同步重入调用本类中的 Native 方法**，
 *    否则可能导致底层的锁递归死锁。
 * 2. **返回值约定**：除返回字符串的查询方法外，绝大多数返回 [Int] 的接口遵循统一定义：返回 `0` 表示操作成功；返回负数表示具体错误码
 *    （参见 [xin.cosmos.bolt.model.ErrorMessages] 及 `crates/utils/src/error.rs` 的 `BtError`）。
 * 3. **字符串内存管理**：对于返回 [String] 的 Native 方法，底层 Rust 堆分配的 C 字符串已由 JNI 桥接层自动转换为 Java 字符串并释放底层指针，
 *    上层通常无需手动调用 [btFreeString]。
 */
object Native {

    init {
        System.loadLibrary("bt_ffi")
    }

    // =========================================================================
    // 事件类型常量（与 C 头文件 EVT_* 宏定义一致）
    // =========================================================================

    /**
     * 事件 1：局域网设备列表变动通知。
     *
     * 当广播/NSD 发现新增设备、设备更新或设备超时下线时触发。
     * 上层收到此事件后通常调用 [btGetDevices] 拉取最新设备列表。
     */
    const val EVT_DEVICE_LIST = 1

    /**
     * 事件 2：设备连接状态变动通知。
     *
     * 负载 JSON 包含：
     * - `uuid`: 目标设备 UUID
     * - `state`: 连接状态，`"connected"` 或 `"disconnected"`
     * - `transport`: 实际采用的传输协议，`"quic"` 或 `"tcp"`
     * - `err`: 错误码（断开连接且发生异常时非 0）
     * - `name`: 对端设备名称（可选）
     */
    const val EVT_CONN_STATE = 2

    /**
     * 事件 3：收到对端发起的配对请求。
     *
     * 负载 JSON 包含：
     * - `pair_id`: 配对请求全局唯一 ID（Long）
     * - `uuid`: 请求方设备 UUID
     * - `name`: 请求方设备名称
     * - `code`: 六位数字配对验证码，供用户双端跨屏比对
     *
     * 用户确认比对结果后，需调用 [btRespondPair] 进行应答（同意/拒绝）。
     */
    const val EVT_PAIR_REQUEST = 3

    /**
     * 事件 4：收到对端发起的入站文件传输请求。
     *
     * 负载 JSON 包含：
     * - `req_id`: 传输请求全局唯一 ID（Long）
     * - `uuid`: 发送方设备 UUID
     * - `name`: 发送方设备名称
     * - `file_count`: 待传输的文件总数
     * - `total_size`: 待传输的总字节数
     *
     * 用户在 UI 弹窗选择接收或拒绝后，需调用 [btRespondTransfer] 进行应答。
     */
    const val EVT_TRANSFER_REQUEST = 4

    /**
     * 事件 5：传输任务状态变动通知。
     *
     * 负载 JSON 包含：
     * - `task_id`: 任务 ID（Long）
     * - `state`: 任务最新状态（`waiting_accept`, `transferring`, `done`, `cancelled`, `error`, `rejected`）
     * - `incoming`: 是否为接收任务（Boolean，true 为接收，false 为发送）
     */
    const val EVT_TASK_STATE = 5

    /**
     * 事件 6：传输进度更新通知（带频控限流）。
     *
     * 负载 JSON 包含：
     * - `task_id`: 任务 ID（Long）
     * - `incoming`: 是否为接收任务（Boolean）
     * - `done`: 当前已完成传输的字节数（Long）
     * - `total`: 任务总字节数（Long）
     * - `rate_bps`: 当前瞬时传输速率（字节/秒）
     * - `eta_secs`: 预计剩余耗时（秒）
     * - `rel_path`: 当前正在传输的单文件相对路径
     */
    const val EVT_TASK_PROGRESS = 6

    /**
     * 事件 7：传输任务完成汇总通知。
     *
     * 当任务整体传输流程结束时触发。负载 JSON 包含：
     * - `task_id`: 任务 ID（Long）
     * - `incoming`: 是否为接收任务（Boolean）
     * - `ok`: 成功传输的文件数量（Int）
     * - `failed`: 传输失败的文件数量（Int）
     */
    const val EVT_TASK_SUMMARY = 7

    /**
     * 事件 8：全局异常或异步操作错误通知。
     *
     * 负载 JSON 包含：
     * - `code`: 错误码（负整数，参见 [xin.cosmos.bolt.model.ErrorMessages]）
     * - `message`: 详细错误信息或排查提示
     */
    const val EVT_ERROR = 8

    // =========================================================================
    // 引擎生命周期与全局配置
    // =========================================================================

    /**
     * 初始化 Bolt 核心引擎（包括网络运行时、传输协议栈、存储与发现服务）。
     *
     * 此方法具有幂等性，重复调用直接返回 0。
     *
     * @param dataDir 本地数据私有存储目录绝对路径（如 Android 内部存储 `context.filesDir/bolt`）。若为 null 则采用系统默认目录。
     * @return `0` 表示成功；负数表示错误码。
     */
    external fun btInit(dataDir: String?): Int

    /**
     * 关闭 Bolt 核心引擎并释放底层全部资源（停止发现服务、关闭网络监听与所有连接、退出后台运行时）。
     *
     * 此操作具有幂等性，安全支持多次调用。
     * 关闭前会自动注销全局事件回调，防止野指针回调。
     */
    external fun btShutdown()

    /**
     * 获取底层 Rust 核心库的版本号。
     *
     * @return 静态版本字符串（例如 `"0.1.0"`）。
     */
    external fun btVersion(): String

    /**
     * 注册全局事件回调函数。
     *
     * @param cb 事件监听回调接口实例 [BtEventCallback]。传入 null 表示清除已注册的回调。
     *           注意：回调在 Rust 专属事件线程中被调用，严禁在回调内重入调用 Native 接口。
     */
    external fun btSetEventCallback(cb: BtEventCallback?)

    /**
     * 增量合并并更新配置项。
     *
     * 仅更新传入 JSON 中包含的字段，未提及的字段保持原值不变。
     *
     * @param json 包含待更新字段的 JSON 字符串，例如：`{"device_name": "我的手机", "stealth_mode": false}`。
     * @return `0` 表示成功；`-1` 表示 JSON 解析失败或参数非法。
     */
    external fun btSetConfig(json: String): Int

    /**
     * 获取当前全量配置信息。
     *
     * 包含设备名称、监听端口、存储路径、隐身模式、自动接收可信设备、并发数等字段。
     *
     * @return 全量配置的 JSON 字符串。
     */
    external fun btGetConfig(): String

    /**
     * 获取本机 TLS 证书的 SHA-256 指纹。
     *
     * 格式为冒号分隔的大写十六进制字符串（例如 `"AA:BB:CC:..."`），供两端配对时人工屏幕核对以防中间人攻击。
     *
     * @return 证书指纹字符串。
     */
    external fun btGetLocalFingerprint(): String

    /**
     * 获取本机设备信息。
     *
     * 返回信息包含本机的 `uuid`、`name`、`dt` (设备类型)、`qport` (QUIC监听端口)、`tport` (TCP监听端口)、`ver` (协议版本)、`stealth` (隐身状态) 等。
     * 在 Android 平台中，Kotlin 层通过读取该信息作为元数据注册 Android 原生 `NsdManager` 服务。
     *
     * @return 包含本机设备元数据的 JSON 字符串。
     */
    external fun btGetLocalInfo(): String

    // =========================================================================
    // 设备发现与网络探测
    // =========================================================================

    /**
     * 启动局域网设备发现服务（监听 UDP 广播及探测响应）。
     *
     * @return `0` 表示成功；负数表示错误码。
     */
    external fun btStartDiscovery(): Int

    /**
     * 停止局域网设备发现服务。
     *
     * @return `0` 表示成功；负数表示错误码。
     */
    external fun btStopDiscovery(): Int

    /**
     * 触发主动网络探测（主动向局域网发送探测广播包以快速唤醒与刷新在线设备）。
     *
     * @return `0` 表示成功；负数表示错误码。
     */
    external fun btProbeNetwork(): Int

    /**
     * 获取当前已发现的局域网在线设备列表快照。
     *
     * @return 设备对象数组的 JSON 字符串（每个设备包含 `uuid`, `name`, `ip`, `quicPort`, `tcpPort`, `deviceType` 等）。
     */
    external fun btGetDevices(): String

    /**
     * 手动添加目标设备（用于直连场景，当组播/广播被局域网 AP 隔离屏蔽时可直接通过 IP/端口添加）。
     *
     * @param ip 目标设备的 IPv4 或 IPv6 地址字符串。
     * @param port 目标设备的监听端口。
     * @return `0` 表示成功；`-1` 表示参数非法。
     */
    external fun btAddManualDevice(ip: String, port: Int): Int

    /**
     * 向 Rust 核心设备表中注入 Android 原生 `NsdManager` 发现的对端设备信息。
     *
     * 由于 Android 环境下 mDNS 组播直接收发受系统节电策略及权限限制，Android 端通常借助原生 `NsdManager` 浏览 `_bolt._udp.` 服务，
     * 解析到设备属性后通过该方法注入核心层，以便与 PC 端等无缝对齐。
     *
     * @param json 包含设备属性的 JSON 字符串，字段必须包含 `uuid`, `ip`, `name`, `dt`, `qport`, `tport`, `ver`, `ptcp`。
     * @return `0` 表示注入成功；`-1` 表示 JSON 参数不合法或缺失必要字段。
     */
    external fun btNsdInjectDevice(json: String): Int

    /**
     * 通知 Rust 核心设备表移除指定的 NSD 发现设备。
     *
     * 当 Android 原生 `NsdManager` 收到服务下线或丢失（`onServiceLost`）回调时调用此方法同步清理。
     *
     * @param uuid 离线设备的 UUID 字符串。
     * @return `0` 表示成功；负数表示错误码。
     */
    external fun btNsdRemoveDevice(uuid: String): Int

    // =========================================================================
    // 设备连接与配对协商
    // =========================================================================

    /**
     * 根据设备 UUID 发起连接（异步操作）。
     *
     * 优先尝试 QUIC 协议连接，失败时自动尝试回落到 TCP。连接结果将通过 [EVT_CONN_STATE] 或 [EVT_ERROR] 事件异步回调通知。
     *
     * @param uuid 目标设备 UUID（必须已存在于已知设备表中）。
     * @return `0` 表示连接请求已成功发起；负数表示错误码（如目标不存在或网络栈未就绪）。
     */
    external fun btConnect(uuid: String): Int

    /**
     * 直接通过指定的 IP 和端口建立连接（异步操作）。
     *
     * 用于跨网段或广播不可达环境下的指定地址直连。连接结果通过 [EVT_CONN_STATE] 或 [EVT_ERROR] 事件异步回调。
     *
     * @param ip 目标 IP 地址。
     * @param port 目标端口。
     * @return `0` 表示连接请求已成功发起；负数表示错误码。
     */
    external fun btConnectAddr(ip: String, port: Int): Int

    /**
     * 主动断开与指定设备的网络连接。
     *
     * @param uuid 目标设备的 UUID。
     * @return `0` 表示成功；负数表示错误码。
     */
    external fun btDisconnect(uuid: String): Int

    /**
     * 响应对端发起的配对请求。
     *
     * 在收到 [EVT_PAIR_REQUEST] 事件后，由用户确认双端验证码是否一致，并通过此接口应答。
     *
     * @param pairId 配对请求 ID（对应 [EVT_PAIR_REQUEST] 中的 `pair_id`）。
     * @param accept 应答决策：`1` 表示同意配对；`0` 表示拒绝配对。
     * @return `0` 表示成功；负数表示错误码。
     */
    external fun btRespondPair(pairId: Long, accept: Int): Int

    /**
     * 响应对端发起的入站文件传输请求。
     *
     * 在收到 [EVT_TRANSFER_REQUEST] 事件后，由用户决定是否接收对方传输的文件。
     *
     * @param reqId 传输请求 ID（对应 [EVT_TRANSFER_REQUEST] 中的 `req_id`）。
     * @param accept 应答决策：`1` 表示同意接收；`0` 表示拒绝接收。
     * @return `0` 表示成功；负数表示错误码。
     */
    external fun btRespondTransfer(reqId: Long, accept: Int): Int

    // =========================================================================
    // 文件传输任务管理
    // =========================================================================

    /**
     * 向目标设备发起文件/文件夹发送任务。
     *
     * 核心库将遍历所有待发路径生成元数据清单并向对端发起协商。
     *
     * @param uuid 接收方设备 UUID。
     * @param pathsJson 包含待发送本地绝对路径列表的 JSON 数组字符串，例如：`"[\"/storage/emulated/0/...\", \"...\"]"`。
     * @param outTaskId 输出参数，长度至少为 1 的 [LongArray]。当方法返回成功（`0`）时，新建任务的 ID 将写入下标 `0` 处。
     * @return `0` 表示任务创建成功；负数表示错误码（例如文件不存在、参数无效、设备未连接等）。
     */
    external fun btSendFiles(uuid: String, pathsJson: String, outTaskId: LongArray): Int

    /**
     * 取消正在进行或等待中的传输任务。
     *
     * 取消后任务状态将流转为 `cancelled`，相关连接与传输通道将被关闭并释放对应资源。
     *
     * @param taskId 待取消的任务 ID。
     * @return `0` 表示取消成功；负数表示错误码。
     */
    external fun btCancelTask(taskId: Long): Int

    /**
     * 获取当前所有传输任务的快照列表。
     *
     * 包含所有活动中（等待同意、传输中）以及已终结（已完成、已取消、失败、被拒绝）的任务信息。
     *
     * @return 任务对象数组的 JSON 字符串。
     */
    external fun btGetTasks(): String

    /**
     * 清理任务列表中已处于终结状态的历史任务记录（状态为 `done`, `cancelled`, `error`, `rejected` 的记录）。
     *
     * @return `0` 表示成功；负数表示错误码。
     */
    external fun btClearRecords(): Int

    /**
     * 清理底层临时缓存（包括未完成的临时传输文件及暂存文件等）。
     *
     * @return `0` 表示成功；负数表示错误码。
     */
    external fun btClearTempCache(): Int

    // =========================================================================
    // 内存释放
    // =========================================================================

    /**
     * 释放由 Rust 核心层动态分配在堆内存上的 C 字符串。
     *
     * 注意：本类中返回 [String] 的 JNI 方法在 JNI 层内部已自动完成 Java 字符串拷贝与 native 指针释放，
     * 此接口主要用于底层裸指针交互或特定拓展场景。
     *
     * @param ptr 待释放的 C 字符串本地指针地址（`*mut c_char`）。
     */
    external fun btFreeString(ptr: Long)
}
