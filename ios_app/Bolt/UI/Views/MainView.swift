//
//  MainView.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

/// Bolt iOS 主容器脚手架视图。
public struct MainView: View {

    @ObservedObject private var engine = BoltEngine.shared
    @State private var selectedTab: Int = 0
    @State private var isSettingsPresented: Bool = false
    @State private var isLogsPresented: Bool = false

    public init() {}

    private var activeTaskCount: Int {
        engine.uiState.tasks.values.filter { TaskStates.isActive($0.state) }.count
    }

    private var activePairRequest: PendingDialog? {
        engine.uiState.pendingDialogs.first {
            if case .pairRequest = $0 { return true }
            return false
        }
    }

    private var activeTransferRequest: PendingDialog? {
        engine.uiState.pendingDialogs.first {
            if case .transferRequest = $0 { return true }
            return false
        }
    }

    public var body: some View {
        NavigationStack {
            ZStack(alignment: .bottom) {
                // 主 Tab 内容
                TabView(selection: $selectedTab) {
                    DevicesView()
                        .tabItem {
                            Label("发现", systemImage: "antenna.radiowaves.left.and.right")
                        }
                        .tag(0)

                    TransfersView()
                        .tabItem {
                            Label("传输", systemImage: "arrow.up.arrow.down.circle")
                        }
                        .badge(activeTaskCount > 0 ? "\(activeTaskCount)" : nil)
                        .tag(1)
                }
                .tint(BoltTheme.electricBlueLight)

                // 全局浮动 Toast 提示
                if let msg = engine.toastMessage {
                    VStack {
                        Spacer()
                        HStack(spacing: 8) {
                            Image(systemName: "info.circle.fill")
                                .foregroundColor(BoltTheme.cyberTeal)
                            Text(msg)
                                .font(.system(size: 13, weight: .medium))
                                .foregroundColor(.white)
                        }
                        .padding(.horizontal, 16)
                        .padding(.vertical, 12)
                        .background(
                            Capsule()
                                .fill(Color(hex: "#1E293B").opacity(0.95))
                                .overlay(Capsule().stroke(BoltTheme.borderHighlight, lineWidth: 1))
                                .shadow(color: Color.black.opacity(0.4), radius: 10, y: 5)
                        )
                        .padding(.bottom, 60)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                    }
                    .animation(.spring(), value: engine.toastMessage)
                    .onAppear {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) {
                            engine.toastMessage = nil
                        }
                    }
                }
            }
            .navigationTitle(selectedTab == 0 ? "设备发现" : "传输管理")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    HStack(spacing: 6) {
                        Image(systemName: "bolt.fill")
                            .font(.system(size: 16, weight: .bold))
                            .foregroundStyle(BoltTheme.brandGradient)
                        Text("Bolt")
                            .font(.system(size: 18, weight: .heavy))
                            .foregroundColor(BoltTheme.textPrimary)
                    }
                }

                ToolbarItem(placement: .navigationBarTrailing) {
                    HStack(spacing: 14) {
                        Button {
                            isLogsPresented = true
                        } label: {
                            Image(systemName: "clock.arrow.circlepath")
                                .font(.system(size: 16))
                                .foregroundColor(BoltTheme.textSecondary)
                        }

                        Button {
                            isSettingsPresented = true
                        } label: {
                            Image(systemName: "gearshape.fill")
                                .font(.system(size: 16))
                                .foregroundColor(BoltTheme.textSecondary)
                        }
                    }
                }
            }
            .sheet(isPresented: $isSettingsPresented) {
                SettingsView()
            }
            .sheet(isPresented: $isLogsPresented) {
                TransferLogsView()
            }
            .sheet(item: Binding<PendingDialog?>(
                get: { activePairRequest },
                set: { _ in }
            )) { dialog in
                if case .pairRequest(let pairId, let uuid, let name, let code) = dialog {
                    PairingDialogView(pairId: pairId, uuid: uuid, name: name, code: code)
                }
            }
            .sheet(item: Binding<PendingDialog?>(
                get: { activeTransferRequest },
                set: { _ in }
            )) { dialog in
                if case .transferRequest(let reqId, let uuid, let name, let fileCount, let totalSize) = dialog {
                    TransferRequestDialogView(reqId: reqId, uuid: uuid, name: name, fileCount: fileCount, totalSize: totalSize)
                }
            }
        }
    }
}
