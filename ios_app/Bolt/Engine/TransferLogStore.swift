//
//  TransferLogStore.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import Foundation

/// 传输审计历史日志本地持久化仓储。
public final class TransferLogStore: ObservableObject {

    public static let shared = TransferLogStore()

    @Published public private(set) var logs: [TransferLogUi] = []

    private let fileManager = FileManager.default
    private let logFileUrl: URL

    private init() {
        let docs = fileManager.urls(for: .documentDirectory, in: .userDomainMask)[0]
        self.logFileUrl = docs.appendingPathComponent("transfer_logs.json")
        loadLogs()
    }

    public func loadLogs() {
        guard fileManager.fileExists(atPath: logFileUrl.path),
              let data = try? Data(contentsOf: logFileUrl),
              let loaded = try? JSONDecoder().decode([TransferLogUi].self, from: data) else {
            self.logs = []
            return
        }
        self.logs = loaded.sorted { $0.startTimeMs > $1.startTimeMs }
    }

    public func appendLog(_ log: TransferLogUi) {
        var current = self.logs
        current.insert(log, at: 0)
        // 最多保留 200 条
        if current.count > 200 {
            current = Array(current.prefix(200))
        }
        self.logs = current
        saveLogs()
    }

    public func clearLogs() {
        self.logs = []
        try? fileManager.removeItem(at: logFileUrl)
    }

    private func saveLogs() {
        guard let data = try? JSONEncoder().encode(self.logs) else { return }
        try? data.write(to: logFileUrl, options: .atomic)
    }
}
