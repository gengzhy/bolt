//
//  SettingsView.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

/// 系统设置与应用首选项管理视图。
public struct SettingsView: View {

    @Environment(\.dismiss) private var dismiss
    @ObservedObject private var engine = BoltEngine.shared

    @State private var deviceName: String = ""
    @State private var stealthMode: Bool = false
    @State private var autoAcceptTrusted: Bool = false
    @State private var preferQuic: Bool = true
    @State private var concurrency: Int = 4

    public init() {}

    public var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 16) {
                    // 设备基本信息
                    deviceSection

                    // 传输与协议设置
                    protocolSection

                    // 存储与沙盒
                    storageSection

                    // 隐私与安全
                    securitySection

                    // 关于 Bolt
                    aboutSection
                }
                .padding()
            }
            .background(BoltTheme.bgPrimary.ignoresSafeArea())
            .navigationTitle("设置")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("完成") {
                        saveSettings()
                        dismiss()
                    }
                    .foregroundColor(BoltTheme.electricBlueLight)
                }
            }
            .onAppear {
                loadCurrentSettings()
            }
        }
    }

    private var deviceSection: some View {
        BtCard {
            VStack(alignment: .leading, spacing: 14) {
                Text("设备标识")
                    .font(.system(size: 13, weight: .bold))
                    .foregroundColor(BoltTheme.cyberTeal)

                VStack(alignment: .leading, spacing: 6) {
                    Text("设备显示名称")
                        .font(.system(size: 12))
                        .foregroundColor(BoltTheme.textMuted)
                    TextField("我的设备", text: $deviceName)
                        .padding(10)
                        .background(BoltTheme.bgInput)
                        .cornerRadius(8)
                        .foregroundColor(BoltTheme.textPrimary)
                }

                VStack(alignment: .leading, spacing: 6) {
                    Text("本机证书指纹 (BLAKE3)")
                        .font(.system(size: 12))
                        .foregroundColor(BoltTheme.textMuted)
                    HStack {
                        Text(engine.uiState.fingerprint.isEmpty ? "--" : engine.uiState.fingerprint)
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundColor(BoltTheme.textSecondary)
                            .lineLimit(1)
                        Spacer()
                        Button {
                            UIPasteboard.general.string = engine.uiState.fingerprint
                            engine.toastMessage = "指纹已复制"
                        } label: {
                            Image(systemName: "doc.on.doc")
                                .font(.system(size: 13))
                                .foregroundColor(BoltTheme.electricBlueLight)
                        }
                    }
                    .padding(10)
                    .background(BoltTheme.bgInput)
                    .cornerRadius(8)
                }
            }
        }
    }

    private var protocolSection: some View {
        BtCard {
            VStack(alignment: .leading, spacing: 14) {
                Text("传输协议")
                    .font(.system(size: 13, weight: .bold))
                    .foregroundColor(BoltTheme.cyberTeal)

                Toggle(isOn: $preferQuic) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("优先 QUIC 协议")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundColor(BoltTheme.textPrimary)
                        Text("基于 UDP + TLS 1.3 极速传输，抗丢包能力强")
                            .font(.system(size: 11))
                            .foregroundColor(BoltTheme.textMuted)
                    }
                }
                .tint(BoltTheme.electricBlue)

                Stepper(value: $concurrency, in: 1...8) {
                    HStack {
                        Text("并发传输通道数: \(concurrency)")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundColor(BoltTheme.textPrimary)
                    }
                }
            }
        }
    }

    private var storageSection: some View {
        BtCard {
            VStack(alignment: .leading, spacing: 14) {
                Text("存储位置")
                    .font(.system(size: 13, weight: .bold))
                    .foregroundColor(BoltTheme.cyberTeal)

                VStack(alignment: .leading, spacing: 4) {
                    Text("接收文件保存目录")
                        .font(.system(size: 12))
                        .foregroundColor(BoltTheme.textMuted)
                    Text("我的 iPhone → Bolt → Received")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(BoltTheme.textPrimary)
                }

                Button {
                    openDocumentsDirectory()
                } label: {
                    HStack {
                        Image(systemName: "folder")
                        Text("在系统「文件」App 中浏览")
                    }
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundColor(BoltTheme.cyberTeal)
                    .frame(maxWidth: .infinity)
                    .frame(height: 38)
                    .background(BoltTheme.bgInput)
                    .cornerRadius(10)
                }
            }
        }
    }

    private var securitySection: some View {
        BtCard {
            VStack(alignment: .leading, spacing: 14) {
                Text("隐私与发现")
                    .font(.system(size: 13, weight: .bold))
                    .foregroundColor(BoltTheme.cyberTeal)

                Toggle(isOn: $stealthMode) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("隐身模式")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundColor(BoltTheme.textPrimary)
                        Text("暂停 Bonjour 广播与网络应答，局域网中隐藏自身")
                            .font(.system(size: 11))
                            .foregroundColor(BoltTheme.textMuted)
                    }
                }
                .tint(BoltTheme.cyberTeal)

                Toggle(isOn: $autoAcceptTrusted) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("自动接收受信任设备文件")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundColor(BoltTheme.textPrimary)
                        Text("已完成 4 位配对的设备发起传输时免人工确认")
                            .font(.system(size: 11))
                            .foregroundColor(BoltTheme.textMuted)
                    }
                }
                .tint(BoltTheme.emeraldGreen)
            }
        }
    }

    private var aboutSection: some View {
        BtCard {
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text("Bolt 原生引擎版本")
                        .font(.system(size: 13))
                        .foregroundColor(BoltTheme.textSecondary)
                    Spacer()
                    Text(engine.uiState.version.isEmpty ? "0.1.0" : engine.uiState.version)
                        .font(.system(size: 13, design: .monospaced))
                        .foregroundColor(BoltTheme.textPrimary)
                }

                Divider()
                    .background(BoltTheme.borderSubtle)

                Text("纯局域网 · 零服务器 · 端到端 TLS 1.3 传输加密\n基于 Rust 底层统一核心与原生 Swift 驱动构建。")
                    .font(.system(size: 11))
                    .foregroundColor(BoltTheme.textMuted)
                    .lineSpacing(4)
            }
        }
    }

    private func loadCurrentSettings() {
        let cfg = engine.uiState.config
        deviceName = cfg.deviceName
        stealthMode = cfg.stealthMode
        autoAcceptTrusted = cfg.autoAcceptTrusted
        preferQuic = cfg.preferQuic
        concurrency = cfg.concurrency
    }

    private func saveSettings() {
        var newCfg = engine.uiState.config
        newCfg.deviceName = deviceName.trimmingCharacters(in: .whitespacesAndNewlines)
        newCfg.stealthMode = stealthMode
        newCfg.autoAcceptTrusted = autoAcceptTrusted
        newCfg.preferQuic = preferQuic
        newCfg.concurrency = concurrency
        engine.updateConfig(newConfig: newCfg)
    }

    private func openDocumentsDirectory() {
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        let received = docs.appendingPathComponent("Received")
        try? FileManager.default.createDirectory(at: received, withIntermediateDirectories: true)

        if let shareUrl = URL(string: "shareddocuments://\(received.path)") {
            if UIApplication.shared.canOpenURL(shareUrl) {
                UIApplication.shared.open(shareUrl)
                return
            }
        }
        engine.toastMessage = "接收目录已就绪: Documents/Received"
    }
}
