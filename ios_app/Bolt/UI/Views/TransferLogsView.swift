//
//  TransferLogsView.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

/// 传输审计历史日志视图。
public struct TransferLogsView: View {

    @Environment(\.dismiss) private var dismiss
    @ObservedObject private var logStore = TransferLogStore.shared
    @State private var searchText: String = ""

    public init() {}

    private var filteredLogs: [TransferLogUi] {
        if searchText.isEmpty {
            return logStore.logs
        }
        return logStore.logs.filter {
            $0.fileName.localizedCaseInsensitiveContains(searchText) ||
            $0.peerName.localizedCaseInsensitiveContains(searchText)
        }
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                BoltTheme.bgPrimary.ignoresSafeArea()

                if logStore.logs.isEmpty {
                    emptyState
                } else {
                    logsList
                }
            }
            .navigationTitle("传输审计日志")
            .navigationBarTitleDisplayMode(.inline)
            .searchable(text: $searchText, prompt: "按文件名或设备名搜索")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("关闭") { dismiss() }
                        .foregroundColor(BoltTheme.electricBlueLight)
                }

                if !logStore.logs.isEmpty {
                    ToolbarItem(placement: .destructiveAction) {
                        Button("清空") {
                            logStore.clearLogs()
                        }
                        .foregroundColor(BoltTheme.roseError)
                    }
                }
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 16) {
            Image(systemName: "doc.text.magnifyingglass")
                .font(.system(size: 50))
                .foregroundColor(BoltTheme.textMuted)
            Text("暂无传输记录")
                .font(.system(size: 15))
                .foregroundColor(BoltTheme.textSecondary)
        }
    }

    private var logsList: some View {
        ScrollView {
            LazyVStack(spacing: 12) {
                ForEach(filteredLogs) { log in
                    BtCard {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack {
                                Text(log.fileName.isEmpty ? "批次文件传输" : (log.fileName as NSString).lastPathComponent)
                                    .font(.system(size: 14, weight: .bold))
                                    .foregroundColor(BoltTheme.textPrimary)
                                Spacer()
                                Text(log.state == TaskStates.DONE ? "传输完成" : "传输中断")
                                    .font(.system(size: 11, weight: .semibold))
                                    .foregroundColor(log.state == TaskStates.DONE ? BoltTheme.emeraldGreen : BoltTheme.roseError)
                            }

                            HStack(spacing: 8) {
                                Text(log.incoming ? "来源: \(log.peerName)" : "目标: \(log.peerName)")
                                    .font(.system(size: 12))
                                    .foregroundColor(BoltTheme.textSecondary)
                                Text("•")
                                    .foregroundColor(BoltTheme.textMuted)
                                Text(Formatters.formatBytes(log.totalSize))
                                    .font(.system(size: 12))
                                    .foregroundColor(BoltTheme.cyberTeal)
                                if log.durationMs > 0 {
                                    Text("•")
                                        .foregroundColor(BoltTheme.textMuted)
                                    Text("耗时 \(Formatters.formatDuration(ms: log.durationMs))")
                                        .font(.system(size: 12))
                                        .foregroundColor(BoltTheme.textMuted)
                                }
                            }

                            HStack {
                                Text(Formatters.formatDate(timestampMs: log.endTimeMs))
                                    .font(.system(size: 11))
                                    .foregroundColor(BoltTheme.textMuted)
                                Spacer()
                                if log.avgRateBps > 0 {
                                    Text("平均 \(Formatters.formatSpeed(log.avgRateBps))")
                                        .font(.system(size: 11, design: .monospaced))
                                        .foregroundColor(BoltTheme.emeraldGreen)
                                }
                            }
                        }
                    }
                }
            }
            .padding()
        }
    }
}
