//
//  BoltNative.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import Foundation

/// 事件回调闭包签名：(eventId: Int32, payloadJson: String?)
public typealias BoltEventClosure = (Int32, String?) -> Void

/// 封装底层 Rust C ABI (crates/ffi/include/bt_api.h) 的强类型 Swift 单例。
public final class BoltNative {

    public static let shared = BoltNative()

    // =========================================================================
    // 事件常量定义（与 bt_api.h EVT_* 严格对齐）
    // =========================================================================
    public static let EVT_DEVICE_LIST: Int32 = 1
    public static let EVT_CONN_STATE: Int32 = 2
    public static let EVT_PAIR_REQUEST: Int32 = 3
    public static let EVT_TRANSFER_REQUEST: Int32 = 4
    public static let EVT_TASK_STATE: Int32 = 5
    public static let EVT_TASK_PROGRESS: Int32 = 6
    public static let EVT_TASK_SUMMARY: Int32 = 7
    public static let EVT_ERROR: Int32 = 8

    private static var eventHandler: BoltEventClosure?

    private init() {}

    // =========================================================================
    // 生命周期
    // =========================================================================

    /// 初始化 Bolt 引擎与网络通道。
    /// - Parameter dataDir: 数据存储绝对路径（为 nil 时使用默认目录）。
    /// - Returns: 0 成功 / 负错误码。
    @discardableResult
    public func btInit(dataDir: String? = nil) -> Int32 {
        if let dir = dataDir {
            return dir.withCString { cStr in
                bt_init(cStr)
            }
        } else {
            return bt_init(nil)
        }
    }

    /// 优雅停机并释放底层资源。幂等。
    public func btShutdown() {
        bt_shutdown()
    }

    /// 获取底层核心引擎版本字符串。
    public func btVersion() -> String {
        guard let ptr = bt_version() else { return "0.1.0" }
        return String(cString: ptr)
    }

    // =========================================================================
    // 事件监听
    // =========================================================================

    /// 注册全局事件监听闭包。底层在独立线程产生事件，本方法内部确保分发到主线程以防重入死锁。
    public func setEventHandler(_ handler: @escaping BoltEventClosure) {
        BoltNative.eventHandler = handler

        let cCallback: @convention(c) (Int32, UnsafePointer<CChar>?) -> Void = { eventId, payloadPtr in
            var payloadStr: String? = nil
            if let payloadPtr = payloadPtr {
                payloadStr = String(cString: payloadPtr)
            }
            // 异步投递到主线程，严防在底层事件线程中同步重入调用 bt_* 接口导致死锁
            DispatchQueue.main.async {
                BoltNative.eventHandler?(eventId, payloadStr)
            }
        }

        bt_set_event_callback(cCallback)
    }

    /// 清除事件回调
    public func clearEventHandler() {
        BoltNative.eventHandler = nil
        bt_set_event_callback(nil)
    }

    // =========================================================================
    // 配置
    // =========================================================================

    /// 合并更新配置（JSON 字符串）。
    public func btSetConfig(_ json: String) -> Int32 {
        json.withCString { bt_set_config($0) }
    }

    /// 获取全量配置 JSON 字符串。
    public func btGetConfig() -> String {
        guard let ptr = bt_get_config() else { return "{}" }
        let str = String(cString: ptr)
        bt_free_string(ptr)
        return str
    }

    /// 本机证书 BLAKE3 指纹（冒号分隔，跨屏比对用）。
    public func btGetLocalFingerprint() -> String {
        guard let ptr = bt_get_local_fingerprint() else { return "" }
        let str = String(cString: ptr)
        bt_free_string(ptr)
        return str
    }

    /// 获取本机设备信息 JSON（包含 uuid, name, dt, qport, tport, ver, stealth）。
    public func btGetLocalInfo() -> String {
        guard let ptr = bt_get_local_info() else { return "{}" }
        let str = String(cString: ptr)
        bt_free_string(ptr)
        return str
    }

    // =========================================================================
    // 发现与连接
    // =========================================================================

    public func btStartDiscovery() -> Int32 {
        bt_start_discovery()
    }

    public func btStopDiscovery() -> Int32 {
        bt_stop_discovery()
    }

    public func btProbeNetwork() -> Int32 {
        bt_probe_network()
    }

    /// 当前已发现设备列表（JSON 数组）。
    public func btGetDevices() -> String {
        guard let ptr = bt_get_devices() else { return "[]" }
        let str = String(cString: ptr)
        bt_free_string(ptr)
        return str
    }

    /// 手动添加目标设备（直连场景）。
    public func btAddManualDevice(ip: String, port: UInt16) -> Int32 {
        ip.withCString { bt_add_manual_device($0, port) }
    }

    /// 连接指定 UUID 设备。
    public func btConnect(uuid: String) -> Int32 {
        uuid.withCString { bt_connect($0) }
    }

    /// 直连指定 IP 和端口。
    public func btConnectAddr(ip: String, port: UInt16) -> Int32 {
        ip.withCString { bt_connect_addr($0, port) }
    }

    /// 断开与指定设备会话。
    public func btDisconnect(uuid: String) -> Int32 {
        uuid.withCString { bt_disconnect($0) }
    }

    // =========================================================================
    // 配对与传输应答
    // =========================================================================

    /// 应答配对申请。
    public func btRespondPair(pairId: UInt64, accept: Bool) -> Int32 {
        bt_respond_pair(pairId, accept ? 1 : 0)
    }

    /// 应答入站传输申请。
    public func btRespondTransfer(reqId: UInt64, accept: Bool) -> Int32 {
        bt_respond_transfer(reqId, accept ? 1 : 0)
    }

    /// 发送本地文件/目录。
    /// - Parameters:
    ///   - uuid: 目标设备 UUID。
    ///   - pathsJson: 本地绝对路径数组的 JSON 字符串（例如 `["/path/a.txt", "/path/b.png"]`）。
    /// - Returns: (返回码，任务ID)。成功时返回码为 0 且任务 ID 有效。
    public func btSendFiles(uuid: String, pathsJson: String) -> (code: Int32, taskId: UInt64) {
        var taskId: UInt64 = 0
        let code = uuid.withCString { uuidPtr in
            pathsJson.withCString { jsonPtr in
                bt_send_files(uuidPtr, jsonPtr, &taskId)
            }
        }
        return (code, taskId)
    }

    /// 取消指定任务。
    public func btCancelTask(taskId: UInt64) -> Int32 {
        bt_cancel_task(taskId)
    }

    /// 当前全部任务列表（JSON 数组）。
    public func btGetTasks() -> String {
        guard let ptr = bt_get_tasks() else { return "[]" }
        let str = String(cString: ptr)
        bt_free_string(ptr)
        return str
    }

    /// 清除已结束的任务历史记录。
    public func btClearRecords() -> Int32 {
        bt_clear_records()
    }

    /// 清理临时分片缓存。
    public func btClearTempCache() -> Int32 {
        bt_clear_temp_cache()
    }

    // =========================================================================
    // 移动端网络桥 (Bonjour / NSD 注入与移除)
    // =========================================================================

    /// 注入由 iOS Bonjour (NetService / NWBrowser) 发现的设备信息 JSON。
    public func btInjectDevice(json: String) -> Int32 {
        json.withCString { bt_nsd_inject_device($0) }
    }

    /// Bonjour 服务丢失通知。
    public func btRemoveDevice(uuid: String) -> Int32 {
        uuid.withCString { bt_nsd_remove_device($0) }
    }
}
