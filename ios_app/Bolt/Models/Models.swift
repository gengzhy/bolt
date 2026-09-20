//
//  Models.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import Foundation

// MARK: - 连接状态枚举
public enum ConnectionState: String, Codable {
    case none = "none"
    case connecting = "connecting"
    case connected = "connected"
    case disconnected = "disconnected"
}

// MARK: - 设备类型枚举
public enum DeviceType: Int, Codable {
    case unknown = 0
    case windows = 1
    case android = 2
    case ios = 3
    case linux = 4
    case macos = 5

    public var title: String {
        switch self {
        case .windows: return "电脑 (Windows)"
        case .android: return "手机 (Android)"
        case .ios: return "iPhone / iPad"
        case .linux: return "电脑 (Linux)"
        case .macos: return "Mac"
        case .unknown: return "网络设备"
        }
    }

    public var systemIcon: String {
        switch self {
        case .windows: return "desktopcomputer"
        case .android: return "candybarphone"
        case .ios: return "iphone"
        case .linux: return "server.rack"
        case .macos: return "laptopcomputer"
        case .unknown: return "network"
        }
    }
}

// MARK: - 局域网设备模型
public struct DeviceUi: Identifiable, Equatable, Hashable {
    public var id: String { uuid }
    public let uuid: String
    public var name: String
    public var ip: String
    public var quicPort: Int
    public var tcpPort: Int
    public var deviceType: Int
    public var stealth: Bool
    public var source: String
    public var connState: ConnectionState
    public var transport: String

    public init(
        uuid: String,
        name: String,
        ip: String,
        quicPort: Int = 8899,
        tcpPort: Int = 8899,
        deviceType: Int = 0,
        stealth: Bool = false,
        source: String = "",
        connState: ConnectionState = .none,
        transport: String = ""
    ) {
        self.uuid = uuid
        self.name = name
        self.ip = ip
        self.quicPort = quicPort
        self.tcpPort = tcpPort
        self.deviceType = deviceType
        self.stealth = stealth
        self.source = source
        self.connState = connState
        self.transport = transport
    }

    public var typeEnum: DeviceType {
        DeviceType(rawValue: deviceType) ?? .unknown
    }
}

// MARK: - 任务状态常量
public struct TaskStates {
    public static let WAITING_ACCEPT = "waiting_accept"
    public static let TRANSFERRING = "transferring"
    public static let PAUSED = "paused"
    public static let DONE = "done"
    public static let CANCELLED = "cancelled"
    public static let ERROR = "error"
    public static let REJECTED = "rejected"

    public static func isActive(_ state: String) -> Bool {
        state == WAITING_ACCEPT || state == TRANSFERRING
    }

    public static func isTerminal(_ state: String) -> Bool {
        state == DONE || state == CANCELLED || state == ERROR || state == REJECTED
    }
}

// MARK: - 传输任务模型
public struct TaskUi: Identifiable, Equatable {
    public var id: UInt64 { taskId }
    public let taskId: UInt64
    public let incoming: Bool
    public var peerUuid: String
    public var peerName: String
    public var fileCount: Int
    public var totalSize: Int64
    public var doneBytes: Int64
    public var okFiles: Int
    public var failedFiles: Int
    public var state: String
    public var currentFile: String
    public var transport: String
    public var rateBps: Int64
    public var etaSecs: Int64
    public var avgRateBps: Int64
    public var durationMs: Int64
    public var startTimeMs: Int64
    public var createdUnix: Int64

    public init(
        taskId: UInt64,
        incoming: Bool,
        peerUuid: String = "",
        peerName: String = "",
        fileCount: Int = 0,
        totalSize: Int64 = 0,
        doneBytes: Int64 = 0,
        okFiles: Int = 0,
        failedFiles: Int = 0,
        state: String = TaskStates.WAITING_ACCEPT,
        currentFile: String = "",
        transport: String = "",
        rateBps: Int64 = 0,
        etaSecs: Int64 = 0,
        avgRateBps: Int64 = 0,
        durationMs: Int64 = 0,
        startTimeMs: Int64 = 0,
        createdUnix: Int64 = 0
    ) {
        self.taskId = taskId
        self.incoming = incoming
        self.peerUuid = peerUuid
        self.peerName = peerName
        self.fileCount = fileCount
        self.totalSize = totalSize
        self.doneBytes = doneBytes
        self.okFiles = okFiles
        self.failedFiles = failedFiles
        self.state = state
        self.currentFile = currentFile
        self.transport = transport
        self.rateBps = rateBps
        self.etaSecs = etaSecs
        self.avgRateBps = avgRateBps
        self.durationMs = durationMs
        self.startTimeMs = startTimeMs
        self.createdUnix = createdUnix
    }

    public var progress: Double {
        guard totalSize > 0 else { return 0.0 }
        return min(1.0, max(0.0, Double(doneBytes) / Double(totalSize)))
    }
}

// MARK: - 传输日志历史模型
public struct TransferLogUi: Identifiable, Codable, Equatable {
    public let id: String
    public let taskId: UInt64
    public let incoming: Bool
    public let transport: String
    public let peerName: String
    public let peerUuid: String
    public let fileName: String
    public let fileCount: Int
    public let totalSize: Int64
    public let avgRateBps: Int64
    public let startTimeMs: Int64
    public let endTimeMs: Int64
    public let durationMs: Int64
    public let state: String
    public let okFiles: Int
    public let failedFiles: Int

    public init(
        id: String = UUID().uuidString,
        taskId: UInt64,
        incoming: Bool,
        transport: String = "",
        peerName: String = "",
        peerUuid: String = "",
        fileName: String = "",
        fileCount: Int = 1,
        totalSize: Int64 = 0,
        avgRateBps: Int64 = 0,
        startTimeMs: Int64 = 0,
        endTimeMs: Int64 = 0,
        durationMs: Int64 = 0,
        state: String = TaskStates.DONE,
        okFiles: Int = 0,
        failedFiles: Int = 0
    ) {
        self.id = id
        self.taskId = taskId
        self.incoming = incoming
        self.transport = transport
        self.peerName = peerName
        self.peerUuid = peerUuid
        self.fileName = fileName
        self.fileCount = fileCount
        self.totalSize = totalSize
        self.avgRateBps = avgRateBps
        self.startTimeMs = startTimeMs
        self.endTimeMs = endTimeMs
        self.durationMs = durationMs
        self.state = state
        self.okFiles = okFiles
        self.failedFiles = failedFiles
    }
}

// MARK: - 待确认交互弹窗模型
public enum PendingDialog: Identifiable, Equatable {
    case pairRequest(pairId: UInt64, uuid: String, name: String, code: String)
    case transferRequest(reqId: UInt64, uuid: String, name: String, fileCount: Int, totalSize: Int64)

    public var id: String {
        switch self {
        case .pairRequest(let pairId, _, _, _):
            return "pair_\(pairId)"
        case .transferRequest(let reqId, _, _, _, _):
            return "transfer_\(reqId)"
        }
    }
}

// MARK: - 应用配置视图模型
public struct ConfigUi: Equatable {
    public var deviceName: String = ""
    public var saveDir: String = ""
    public var stealthMode: Bool = false
    public var autoAcceptTrusted: Bool = false
    public var concurrency: Int = 4
    public var listenPort: Int = 8899
    public var chunkSize: Int64 = 1048576
    public var preferQuic: Bool = true
    public var useMdns: Bool = true
    public var collision: String = "rename"

    public init() {}
}

// MARK: - 统一 UI 顶层状态
public struct UiState: Equatable {
    public var engineReady: Bool = false
    public var initError: Int32 = 0
    public var fingerprint: String = ""
    public var version: String = ""
    public var devices: [DeviceUi] = []
    public var scanning: Bool = false
    public var tasks: [UInt64: TaskUi] = [:]
    public var pendingDialogs: [PendingDialog] = []
    public var config: ConfigUi = ConfigUi()
    public var localIps: [String] = []
    public var localPort: Int = 8899
    public var staging: Bool = false

    public init() {}
}
