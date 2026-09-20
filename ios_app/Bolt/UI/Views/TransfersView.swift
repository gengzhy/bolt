//
//  TransfersView.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

/// 传输任务状态与历史记录管理视图。
public struct TransfersView: View {

    @ObservedObject private var engine = BoltEngine.shared
    @ObservedObject private var logStore = TransferLogStore.shared
    @State private var selectedTab: Int = 0 // 0: 进行中, 1: 历史记录

    public init() {}

    private var activeTasks: [TaskUi] {
        engine.uiState.tasks.values
            .filter { TaskStates.isActive($0.state) }
            .sorted { $0.taskId > $1.taskId }
    }

    public var body: some View {
        VStack(spacing: 0) {
            // 分段选择器
            Picker("传输标签", selection: $selectedTab) {
                Text("进行中 (\(activeTasks.count))").tag(0)
                Text("已完成 (\(logStore.logs.count))").tag(1)
            }
            .pickerStyle(.segmented)
            .padding()

            ScrollView {
                VStack(spacing: 14) {
                    if selectedTab == 0 {
                        activeSection
                    } else {
                        historySection
                    }
                }
                .padding(.horizontal)
                .padding(.bottom, 20)
            }
        }
        .background(BoltTheme.bgPrimary.ignoresSafeArea())
    }

    // MARK: - 进行中任务区
    private var activeSection: some View {
        VStack(spacing: 14) {
            if activeTasks.isEmpty {
                VStack(spacing: 16) {
                    Spacer().frame(height: 60)
                    Image(systemName: "arrow.up.arrow.down.circle")
                        .font(.system(size: 54))
                        .foregroundColor(BoltTheme.textMuted)
                    Text("当前没有进行中的传输任务")
                        .font(.system(size: 15))
                        .foregroundColor(BoltTheme.textSecondary)
                    Spacer()
                }
            } else {
                ForEach(activeTasks) { task in
                    activeTaskCard(task: task)
                }
            }
        }
    }

    private func activeTaskCard(task: TaskUi) -> some View {
        BtCard {
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 12) {
                    ZStack {
                        Circle()
                            .fill(task.incoming ? BoltTheme.cyberTeal.opacity(0.15) : BoltTheme.electricBlue.opacity(0.15))
                            .frame(width: 40, height: 40)
                        Image(systemName: task.incoming ? "arrow.down.circle.fill" : "arrow.up.circle.fill")
                            .font(.system(size: 20))
                            .foregroundColor(task.incoming ? BoltTheme.cyberTeal : BoltTheme.electricBlueLight)
                    }

                    VStack(alignment: .leading, spacing: 3) {
                        Text(task.incoming ? "接收自: \(task.peerName.isEmpty ? "对端设备" : task.peerName)" : "发送至: \(task.peerName.isEmpty ? "对端设备" : task.peerName)")
                            .font(.system(size: 15, weight: .bold))
                            .foregroundColor(BoltTheme.textPrimary)

                        Text(task.currentFile.isEmpty ? "正在准备数据..." : (task.currentFile as NSString).lastPathComponent)
                            .font(.system(size: 12))
                            .foregroundColor(BoltTheme.textSecondary)
                            .lineLimit(1)
                    }

                    Spacer()

                    // 取消按钮
                    Button {
                        engine.cancelTask(taskId: task.taskId)
                    } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: 12, weight: .bold))
                            .foregroundColor(BoltTheme.textMuted)
                            .frame(width: 28, height: 28)
                            .background(BoltTheme.bgInput)
                            .clipShape(Circle())
                    }
                }

                // 进度条
                ProgressBar(progress: task.progress)

                // 速率与进度指标
                HStack {
                    Text("\(Formatters.formatBytes(task.doneBytes)) / \(Formatters.formatBytes(task.totalSize))")
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundColor(BoltTheme.textSecondary)

                    Spacer()

                    if task.rateBps > 0 {
                        HStack(spacing: 4) {
                            Image(systemName: "bolt.fill")
                                .font(.system(size: 10))
                                .foregroundColor(BoltTheme.emeraldGreen)
                            Text(Formatters.formatSpeed(task.rateBps))
                                .font(.system(size: 12, weight: .semibold, design: .monospaced))
                                .foregroundColor(BoltTheme.emeraldGreen)
                        }
                    }

                    if task.etaSecs > 0 {
                        Text("剩余 \(Formatters.formatEta(task.etaSecs))")
                            .font(.system(size: 11))
                            .foregroundColor(BoltTheme.textMuted)
                    }
                }
            }
        }
    }

    // MARK: - 历史传输日志区
    private var historySection: some View {
        VStack(spacing: 12) {
            if logStore.logs.isEmpty {
                VStack(spacing: 16) {
                    Spacer().frame(height: 60)
                    Image(systemName: "clock.arrow.circlepath")
                        .font(.system(size: 54))
                        .foregroundColor(BoltTheme.textMuted)
                    Text("暂无传输历史记录")
                        .font(.system(size: 15))
                        .foregroundColor(BoltTheme.textSecondary)
                    Spacer()
                }
            } else {
                HStack {
                    Text("共 \(logStore.logs.count) 条记录")
                        .font(.system(size: 12))
                        .foregroundColor(BoltTheme.textMuted)
                    Spacer()
                    Button("清空历史") {
                        logStore.clearLogs()
                    }
                    .font(.system(size: 12))
                    .foregroundColor(BoltTheme.roseError)
                }
                .padding(.horizontal, 4)

                ForEach(logStore.logs) { log in
                    historyLogCard(log: log)
                }
            }
        }
    }

    private func historyLogCard(log: TransferLogUi) -> some View {
        BtCard {
            HStack(spacing: 12) {
                ZStack {
                    Circle()
                        .fill(log.state == TaskStates.DONE ? BoltTheme.emeraldGreen.opacity(0.15) : BoltTheme.roseError.opacity(0.15))
                        .frame(width: 38, height: 38)
                    Image(systemName: log.state == TaskStates.DONE ? "checkmark" : "exclamationmark")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundColor(log.state == TaskStates.DONE ? BoltTheme.emeraldGreen : BoltTheme.roseError)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text(log.fileName.isEmpty ? "批次文件传输" : (log.fileName as NSString).lastPathComponent)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundColor(BoltTheme.textPrimary)
                        .lineLimit(1)

                    HStack(spacing: 8) {
                        Text(log.incoming ? "来源: \(log.peerName)" : "发往: \(log.peerName)")
                            .font(.system(size: 12))
                            .foregroundColor(BoltTheme.textSecondary)

                        Text("•")
                            .foregroundColor(BoltTheme.textMuted)

                        Text(Formatters.formatBytes(log.totalSize))
                            .font(.system(size: 12))
                            .foregroundColor(BoltTheme.cyberTeal)
                    }
                }

                Spacer()

                VStack(alignment: .trailing, spacing: 4) {
                    Text(Formatters.formatDate(timestampMs: log.endTimeMs))
                        .font(.system(size: 10))
                        .foregroundColor(BoltTheme.textMuted)

                    if log.avgRateBps > 0 {
                        Text(Formatters.formatSpeed(log.avgRateBps))
                            .font(.system(size: 11, weight: .medium, design: .monospaced))
                            .foregroundColor(BoltTheme.emeraldGreen)
                    }
                }
            }
        }
    }
}
