package com.lt.transfer.ffi

/** 事件回调签名：void cb(int32_t event_id, const char *payload_json) */
typealias LtEventCallback = (Int, String?) -> Unit

/**
 * liblt_ffi.so 的 JNI 声明（与 crates/lt-ffi/include/lt_api.h 一一对应）。
 *
 * 线程模型：事件回调在 Rust 专用分发线程执行，回调内只做转存
 * （发到主线程 Handler / 协程 Channel），禁止在回调里重入调用本类方法。
 */
object Native {

    init {
        System.loadLibrary("lt_ffi")
    }

    // 事件号（与 EVT_* 宏一致）
    const val EVT_DEVICE_LIST = 1
    const val EVT_CONN_STATE = 2
    const val EVT_PAIR_REQUEST = 3
    const val EVT_TRANSFER_REQUEST = 4
    const val EVT_TASK_STATE = 5
    const val EVT_TASK_PROGRESS = 6
    const val EVT_TASK_SUMMARY = 7
    const val EVT_ERROR = 8

    external fun ltInit(dataDir: String?): Int
    external fun ltShutdown()
    external fun ltVersion(): String
    external fun ltSetEventCallback(cb: LtEventCallback?)
    external fun ltSetConfig(json: String): Int
    external fun ltGetConfig(): String
    external fun ltGetLocalFingerprint(): String
    external fun ltGetLocalInfo(): String

    external fun ltStartDiscovery(): Int
    external fun ltStopDiscovery(): Int
    external fun ltProbeNetwork(): Int
    external fun ltGetDevices(): String
    external fun ltAddManualDevice(ip: String, port: Int): Int
    external fun ltNsdInjectDevice(json: String): Int
    external fun ltNsdRemoveDevice(uuid: String): Int

    external fun ltConnect(uuid: String): Int
    external fun ltConnectAddr(ip: String, port: Int): Int
    external fun ltDisconnect(uuid: String): Int
    external fun ltRespondPair(pairId: Long, accept: Int): Int
    external fun ltRespondTransfer(reqId: Long, accept: Int): Int

    external fun ltSendFiles(uuid: String, pathsJson: String, outTaskId: LongArray): Int
    external fun ltPauseTask(taskId: Long): Int
    external fun ltResumeTask(taskId: Long): Int
    external fun ltCancelTask(taskId: Long): Int
    external fun ltGetTasks(): String
    external fun ltClearRecords(): Int
    external fun ltClearTempCache(): Int

    external fun ltFreeString(ptr: Long)
}
