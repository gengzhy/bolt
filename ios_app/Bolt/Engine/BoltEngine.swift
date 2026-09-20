//
//  BoltEngine.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import Foundation
import UIKit
import PhotosUI
import Combine

/// Bolt iOS 业务调度中枢（驱动全应用状态更新）。
@MainActor
public final class BoltEngine: ObservableObject {

    public static let shared = BoltEngine()

    @Published public private(set) var uiState = UiState()

    /// 跟踪暂存目录映射：taskId -> stagingDir URL
    private var taskStagingDirs: [UInt64: URL] = [:]

    /// 一次性 Toast 提示内容
    @Published public var toastMessage: String?

    private init() {}

    // =========================================================================
    // 生命周期初始化
    // =========================================================================

    public func start() {
        guard !uiState.engineReady else { return }

        // 1. 设置沙盒数据目录
        let docsUrl = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        let dataDirUrl = docsUrl.appendingPathComponent("bolt", isDirectory: true)
        let saveDirUrl = docsUrl.appendingPathComponent("Received", isDirectory: true)
        try? FileManager.default.createDirectory(at: dataDirUrl, withIntermediateDirectories: true)
        try? FileManager.default.createDirectory(at: saveDirUrl, withIntermediateDirectories: true)

        // 2. 调用 Rust 底层初始化
        let initCode = BoltNative.shared.btInit(dataDir: dataDirUrl.path)
        guard initCode == 0 else {
            uiState.initError = initCode
            toastMessage = "核心引擎初始化失败: \(ErrorMessages.of(Int(initCode)))"
            return
        }

        // 3. 注册事件分发
        BoltNative.shared.setEventHandler { [weak self] eventId, payload in
            self?.handleEvent(eventId: eventId, payload: payload)
        }

        // 4. 设置默认设备名称（使用 iOS 机器名，如 "Tom's iPhone"）
        ensureDefaultDeviceName(saveDir: saveDirUrl.path)

        // 5. 刷新静态信息与全量同步
        refreshStaticInfo()
        syncTasks()
        syncDevices()

        // 6. 启动底层发现与 Apple 原生 Bonjour
        BoltNative.shared.btStartDiscovery()
        BonjourService.shared.start()

        // 7. 清扫陈旧缓存
        StagingService.shared.sweepStaleOutbox()

        uiState.engineReady = true
    }

    public func shutdown() {
        BonjourService.shared.stop()
        BoltNative.shared.btStopDiscovery()
        BoltNative.shared.clearEventHandler()
        BoltNative.shared.btShutdown()
        uiState.engineReady = false
    }

    // =========================================================================
    // 事件分发与响应处理
    // =========================================================================

    private func handleEvent(eventId: Int32, payload: String?) {
        guard let payload = payload, let data = payload.data(using: .utf8) else { return }

        switch eventId {
        case BoltNative.EVT_DEVICE_LIST:
            syncDevices()

        case BoltNative.EVT_CONN_STATE:
            handleConnStateEvent(data: data)

        case BoltNative.EVT_PAIR_REQUEST:
            handlePairRequestEvent(data: data)

        case BoltNative.EVT_TRANSFER_REQUEST:
            handleTransferRequestEvent(data: data)

        case BoltNative.EVT_TASK_STATE:
            handleTaskStateEvent(data: data)

        case BoltNative.EVT_TASK_PROGRESS:
            handleTaskProgressEvent(data: data)

        case BoltNative.EVT_TASK_SUMMARY:
            handleTaskSummaryEvent(data: data)

        case BoltNative.EVT_ERROR:
            handleErrorEvent(data: data)

        default:
            break
        }
    }

    private func handleConnStateEvent(data: Data) {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let uuid = obj["uuid"] as? String,
              let stateStr = obj["state"] as? String else { return }

        let transport = obj["transport"] as? String ?? ""
        let errCode = obj["err"] as? Int ?? 0

        let state: ConnectionState
        switch stateStr {
        case "connected": state = .connected
        case "connecting": state = .connecting
        default: state = .disconnected
        }

        // 更新设备列表中的连接状态
        if let idx = uiState.devices.firstIndex(where: { $0.uuid == uuid }) {
            uiState.devices[idx].connState = state
            uiState.devices[idx].transport = transport
        }

        if errCode != 0 {
            toastMessage = "连接异常: \(ErrorMessages.of(errCode))"
        }
    }

    private func handlePairRequestEvent(data: Data) {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let pairId = obj["pair_id"] as? UInt64 ?? (obj["pair_id"] as? NSNumber)?.uint64Value,
              let uuid = obj["uuid"] as? String,
              let name = obj["name"] as? String,
              let code = obj["code"] as? String else { return }

        let dialog = PendingDialog.pairRequest(pairId: pairId, uuid: uuid, name: name, code: code)
        if !uiState.pendingDialogs.contains(where: { $0.id == dialog.id }) {
            uiState.pendingDialogs.append(dialog)
        }
    }

    private func handleTransferRequestEvent(data: Data) {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let reqId = obj["req_id"] as? UInt64 ?? (obj["req_id"] as? NSNumber)?.uint64Value,
              let uuid = obj["uuid"] as? String,
              let name = obj["name"] as? String,
              let fileCount = obj["file_count"] as? Int,
              let totalSize = obj["total_size"] as? Int64 ?? (obj["total_size"] as? NSNumber)?.int64Value else { return }

        let dialog = PendingDialog.transferRequest(reqId: reqId, uuid: uuid, name: name, fileCount: fileCount, totalSize: totalSize)
        if !uiState.pendingDialogs.contains(where: { $0.id == dialog.id }) {
            uiState.pendingDialogs.append(dialog)
        }
    }

    private func handleTaskStateEvent(data: Data) {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let taskId = obj["task_id"] as? UInt64 ?? (obj["task_id"] as? NSNumber)?.uint64Value,
              let state = obj["state"] as? String else { return }

        let incoming = obj["incoming"] as? Bool ?? false

        if var task = uiState.tasks[taskId] {
            task.state = state
            uiState.tasks[taskId] = task
        } else {
            let task = TaskUi(taskId: taskId, incoming: incoming, state: state)
            uiState.tasks[taskId] = task
        }

        // 终态处理：归档至审计日志，清理暂存目录
        if TaskStates.isTerminal(state) {
            if let task = uiState.tasks[taskId] {
                let log = TransferLogUi(
                    taskId: task.taskId,
                    incoming: task.incoming,
                    transport: task.transport,
                    peerName: task.peerName,
                    peerUuid: task.peerUuid,
                    fileName: task.currentFile,
                    fileCount: task.fileCount,
                    totalSize: task.totalSize,
                    avgRateBps: task.avgRateBps,
                    startTimeMs: task.startTimeMs,
                    endTimeMs: Int64(Date().timeIntervalSince1970 * 1000),
                    durationMs: task.durationMs,
                    state: task.state,
                    okFiles: task.okFiles,
                    failedFiles: task.failedFiles
                )
                TransferLogStore.shared.appendLog(log)
            }

            if let stagingDir = taskStagingDirs.removeValue(forKey: taskId) {
                StagingService.shared.cleanup(stagingDir: stagingDir)
            }
        }
    }

    private func handleTaskProgressEvent(data: Data) {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let taskId = obj["task_id"] as? UInt64 ?? (obj["task_id"] as? NSNumber)?.uint64Value else { return }

        let done = obj["done"] as? Int64 ?? (obj["done"] as? NSNumber)?.int64Value ?? 0
        let total = obj["total"] as? Int64 ?? (obj["total"] as? NSNumber)?.int64Value ?? 0
        let rateBps = obj["rate_bps"] as? Int64 ?? (obj["rate_bps"] as? NSNumber)?.int64Value ?? 0
        let etaSecs = obj["eta_secs"] as? Int64 ?? (obj["eta_secs"] as? NSNumber)?.int64Value ?? 0
        let relPath = obj["rel_path"] as? String ?? ""

        if var task = uiState.tasks[taskId] {
            task.doneBytes = done
            if total > 0 { task.totalSize = total }
            task.rateBps = rateBps
            task.etaSecs = etaSecs
            if !relPath.isEmpty { task.currentFile = relPath }
            task.state = TaskStates.TRANSFERRING
            uiState.tasks[taskId] = task
        }
    }

    private func handleTaskSummaryEvent(data: Data) {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let taskId = obj["task_id"] as? UInt64 ?? (obj["task_id"] as? NSNumber)?.uint64Value else { return }

        let ok = obj["ok"] as? Int ?? 0
        let failed = obj["failed"] as? Int ?? 0

        if var task = uiState.tasks[taskId] {
            task.okFiles = ok
            task.failedFiles = failed
            uiState.tasks[taskId] = task
        }
    }

    private func handleErrorEvent(data: Data) {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
        let code = obj["code"] as? Int ?? -15
        let msg = obj["message"] as? String
        toastMessage = ErrorMessages.of(code, fallback: msg)
    }

    // =========================================================================
    // 业务动作接口
    // =========================================================================

    /// 发送选中的文档/本地文件
    public func sendFiles(targetUuid: String, urls: [URL]) async {
        uiState.staging = true
        defer { uiState.staging = false }

        do {
            let staged = try await StagingService.shared.stageFiles(urls: urls)
            guard let pathsData = try? JSONSerialization.data(withJSONObject: staged.paths),
                  let pathsJson = String(data: pathsData, encoding: .utf8) else { return }

            let (code, taskId) = BoltNative.shared.btSendFiles(uuid: targetUuid, pathsJson: pathsJson)
            if code == 0 {
                taskStagingDirs[taskId] = staged.stagingDir
                syncTasks()
                toastMessage = "已发起发送请求"
            } else {
                StagingService.shared.cleanup(stagingDir: staged.stagingDir)
                toastMessage = "发送失败: \(ErrorMessages.of(Int(code)))"
            }
        } catch {
            toastMessage = "文件读取失败: \(error.localizedDescription)"
        }
    }

    /// 发送选中的相册照片/视频
    public func sendPhotos(targetUuid: String, items: [PhotosPickerItem]) async {
        uiState.staging = true
        defer { uiState.staging = false }

        do {
            let staged = try await StagingService.shared.stagePhotosItems(items: items)
            guard let pathsData = try? JSONSerialization.data(withJSONObject: staged.paths),
                  let pathsJson = String(data: pathsData, encoding: .utf8) else { return }

            let (code, taskId) = BoltNative.shared.btSendFiles(uuid: targetUuid, pathsJson: pathsJson)
            if code == 0 {
                taskStagingDirs[taskId] = staged.stagingDir
                syncTasks()
                toastMessage = "已发起发送请求"
            } else {
                StagingService.shared.cleanup(stagingDir: staged.stagingDir)
                toastMessage = "发送失败: \(ErrorMessages.of(Int(code)))"
            }
        } catch {
            toastMessage = "相册资源暂存失败: \(error.localizedDescription)"
        }
    }

    public func connect(uuid: String) {
        let code = BoltNative.shared.btConnect(uuid: uuid)
        if code != 0 {
            toastMessage = "发起连接失败: \(ErrorMessages.of(Int(code)))"
        }
    }

    public func connectAddr(ip: String, port: UInt16) {
        let code = BoltNative.shared.btConnectAddr(ip: ip, port: port)
        if code != 0 {
            toastMessage = "直连失败: \(ErrorMessages.of(Int(code)))"
        } else {
            toastMessage = "正在连接 \(ip):\(port)..."
        }
    }

    public func disconnect(uuid: String) {
        BoltNative.shared.btDisconnect(uuid: uuid)
    }

    public func respondPair(pairId: UInt64, accept: Bool) {
        BoltNative.shared.btRespondPair(pairId: pairId, accept: accept)
        uiState.pendingDialogs.removeAll(where: {
            if case .pairRequest(let pId, _, _, _) = $0 { return pId == pairId }
            return false
        })
    }

    public func respondTransfer(reqId: UInt64, accept: Bool) {
        BoltNative.shared.btRespondTransfer(reqId: reqId, accept: accept)
        uiState.pendingDialogs.removeAll(where: {
            if case .transferRequest(let rId, _, _, _, _) = $0 { return rId == reqId }
            return false
        })
    }

    public func cancelTask(taskId: UInt64) {
        BoltNative.shared.btCancelTask(taskId: taskId)
    }

    public func clearRecords() {
        BoltNative.shared.btClearRecords()
        syncTasks()
    }

    public func probeNetwork() {
        uiState.scanning = true
        BoltNative.shared.btProbeNetwork()
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) { [weak self] in
            self?.uiState.scanning = false
            self?.syncDevices()
        }
    }

    // =========================================================================
    // 状态对账与同步
    // =========================================================================

    public func syncDevices() {
        let json = BoltNative.shared.btGetDevices()
        guard let data = json.data(using: .utf8),
              let list = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else { return }

        var devices: [DeviceUi] = []
        for d in list {
            guard let uuid = d["uuid"] as? String else { continue }
            let name = d["name"] as? String ?? ""
            let ip = d["ip"] as? String ?? ""
            let qport = d["quic_port"] as? Int ?? 8899
            let tport = d["tcp_port"] as? Int ?? 8899
            let dt = d["device_type"] as? Int ?? 0
            let stealth = d["stealth"] as? Bool ?? false
            let source = d["source"] as? String ?? ""

            // 保留既有的连接状态
            let existingConn = uiState.devices.first(where: { $0.uuid == uuid })?.connState ?? .none
            let existingTransport = uiState.devices.first(where: { $0.uuid == uuid })?.transport ?? ""

            devices.append(DeviceUi(
                uuid: uuid,
                name: name,
                ip: ip,
                quicPort: qport,
                tcpPort: tport,
                deviceType: dt,
                stealth: stealth,
                source: source,
                connState: existingConn,
                transport: existingTransport
            ))
        }
        uiState.devices = devices
    }

    public func syncTasks() {
        let json = BoltNative.shared.btGetTasks()
        guard let data = json.data(using: .utf8),
              let list = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else { return }

        var map: [UInt64: TaskUi] = [:]
        for t in list {
            guard let taskId = t["task_id"] as? UInt64 ?? (t["task_id"] as? NSNumber)?.uint64Value else { continue }
            let incoming = t["incoming"] as? Bool ?? false
            let peerUuid = t["peer_uuid"] as? String ?? ""
            let peerName = t["peer_name"] as? String ?? ""
            let fileCount = t["file_count"] as? Int ?? 0
            let totalSize = t["total_size"] as? Int64 ?? (t["total_size"] as? NSNumber)?.int64Value ?? 0
            let doneBytes = t["done_bytes"] as? Int64 ?? (t["done_bytes"] as? NSNumber)?.int64Value ?? 0
            let okFiles = t["ok_files"] as? Int ?? 0
            let failedFiles = t["failed_files"] as? Int ?? 0
            let state = t["state"] as? String ?? TaskStates.WAITING_ACCEPT
            let currentFile = t["current_file"] as? String ?? ""
            let transport = t["transport"] as? String ?? ""
            let rateBps = t["rate_bps"] as? Int64 ?? (t["rate_bps"] as? NSNumber)?.int64Value ?? 0
            let etaSecs = t["eta_secs"] as? Int64 ?? (t["eta_secs"] as? NSNumber)?.int64Value ?? 0
            let avgRateBps = t["avg_rate_bps"] as? Int64 ?? (t["avg_rate_bps"] as? NSNumber)?.int64Value ?? 0
            let durationMs = t["duration_ms"] as? Int64 ?? (t["duration_ms"] as? NSNumber)?.int64Value ?? 0
            let startTimeMs = t["start_time_ms"] as? Int64 ?? (t["start_time_ms"] as? NSNumber)?.int64Value ?? 0
            let createdUnix = t["created_unix"] as? Int64 ?? (t["created_unix"] as? NSNumber)?.int64Value ?? 0

            map[taskId] = TaskUi(
                taskId: taskId,
                incoming: incoming,
                peerUuid: peerUuid,
                peerName: peerName,
                fileCount: fileCount,
                totalSize: totalSize,
                doneBytes: doneBytes,
                okFiles: okFiles,
                failedFiles: failedFiles,
                state: state,
                currentFile: currentFile,
                transport: transport,
                rateBps: rateBps,
                etaSecs: etaSecs,
                avgRateBps: avgRateBps,
                durationMs: durationMs,
                startTimeMs: startTimeMs,
                createdUnix: createdUnix
            )
        }
        uiState.tasks = map
    }

    public func updateConfig(newConfig: ConfigUi) {
        var dict: [String: Any] = [:]
        dict["device_name"] = newConfig.deviceName
        dict["save_dir"] = newConfig.saveDir
        dict["stealth_mode"] = newConfig.stealthMode
        dict["auto_accept_trusted"] = newConfig.autoAcceptTrusted
        dict["concurrency"] = newConfig.concurrency
        dict["prefer_quic"] = newConfig.preferQuic

        if let data = try? JSONSerialization.data(withJSONObject: dict),
           let jsonStr = String(data: data, encoding: .utf8) {
            let code = BoltNative.shared.btSetConfig(jsonStr)
            if code == 0 {
                uiState.config = newConfig
                BonjourService.shared.updateStealthMode(isStealth: newConfig.stealthMode)
                toastMessage = "配置已保存"
            } else {
                toastMessage = "更新配置失败: \(ErrorMessages.of(Int(code)))"
            }
        }
    }

    private func ensureDefaultDeviceName(saveDir: String) {
        let currentCfg = parseConfig(BoltNative.shared.btGetConfig())
        let currentName = currentCfg.deviceName
        let deviceName = UIDevice.current.name.trimmingCharacters(in: .whitespacesAndNewlines)

        let isDefault = currentName.isEmpty ||
            currentName == "device" ||
            currentName.range(of: #"^[A-Za-z0-9._-]+-[0-9a-f]{4}$"#, options: .regularExpression) != nil

        if isDefault && !deviceName.isEmpty {
            let updateDict: [String: Any] = [
                "device_name": deviceName,
                "save_dir": saveDir
            ]
            if let data = try? JSONSerialization.data(withJSONObject: updateDict),
               let json = String(data: data, encoding: .utf8) {
                BoltNative.shared.btSetConfig(json)
            }
        }
    }

    private func refreshStaticInfo() {
        uiState.fingerprint = BoltNative.shared.btGetLocalFingerprint()
        uiState.version = BoltNative.shared.btVersion()
        uiState.config = parseConfig(BoltNative.shared.btGetConfig())

        let localInfoJson = BoltNative.shared.btGetLocalInfo()
        if let data = localInfoJson.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            uiState.localPort = obj["qport"] as? Int ?? 8899
            if let ips = obj["ips"] as? [String] {
                uiState.localIps = ips
            }
        }
    }

    private func parseConfig(_ json: String) -> ConfigUi {
        var cfg = ConfigUi()
        guard let data = json.data(using: .utf8),
              let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return cfg }

        cfg.deviceName = dict["device_name"] as? String ?? ""
        cfg.saveDir = dict["save_dir"] as? String ?? ""
        cfg.stealthMode = dict["stealth_mode"] as? Bool ?? false
        cfg.autoAcceptTrusted = dict["auto_accept_trusted"] as? Bool ?? false
        cfg.concurrency = dict["concurrency"] as? Int ?? 4
        cfg.listenPort = dict["listen_port"] as? Int ?? 8899
        cfg.chunkSize = dict["chunk_size"] as? Int64 ?? 1048576
        cfg.preferQuic = dict["prefer_quic"] as? Bool ?? true
        cfg.useMdns = dict["use_mdns"] as? Bool ?? true
        cfg.collision = dict["collision"] as? String ?? "rename"
        return cfg
    }
}
