//
//  DevicesView.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

/// 局域网在线设备列表与雷达扫描主视图。
public struct DevicesView: View {

    @ObservedObject private var engine = BoltEngine.shared
    @State private var selectedDeviceForShare: DeviceUi?
    @State private var isManualConnectPresented: Bool = false
    @State private var manualIp: String = ""
    @State private var manualPort: String = "8899"

    public init() {}

    public var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                // 本机网络与扫描状态头
                headerCard

                if engine.uiState.devices.isEmpty {
                    emptyRadarState
                } else {
                    devicesList
                }
            }
            .padding()
        }
        .background(BoltTheme.bgPrimary.ignoresSafeArea())
        .sheet(item: $selectedDeviceForShare) { device in
            QuickShareSheet(targetDevice: device)
        }
        .sheet(isPresented: $isManualConnectPresented) {
            manualConnectSheet
        }
    }

    // MARK: - 顶部本机状态卡片
    private var headerCard: some View {
        BtCard {
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 8) {
                        Circle()
                            .fill(BoltTheme.emeraldGreen)
                            .frame(width: 8, height: 8)
                        Text(engine.uiState.config.deviceName.isEmpty ? "本机" : engine.uiState.config.deviceName)
                            .font(.system(size: 16, weight: .bold))
                            .foregroundColor(BoltTheme.textPrimary)
                    }

                    if let primaryIp = engine.uiState.localIps.first {
                        Text("\(primaryIp):\(engine.uiState.localPort)")
                            .font(.system(size: 13, design: .monospaced))
                            .foregroundColor(BoltTheme.textSecondary)
                    } else {
                        Text("监听端口: \(engine.uiState.localPort)")
                            .font(.system(size: 13, design: .monospaced))
                            .foregroundColor(BoltTheme.textSecondary)
                    }
                }

                Spacer()

                HStack(spacing: 10) {
                    // 手动 IP 直连
                    Button {
                        isManualConnectPresented = true
                    } label: {
                        Image(systemName: "plus")
                            .font(.system(size: 14, weight: .bold))
                            .foregroundColor(BoltTheme.cyberTeal)
                            .frame(width: 36, height: 36)
                            .background(BoltTheme.bgInput)
                            .clipShape(Circle())
                    }

                    // 刷新网络广播探测
                    Button {
                        engine.probeNetwork()
                    } label: {
                        Image(systemName: "arrow.clockwise")
                            .font(.system(size: 14, weight: .bold))
                            .foregroundColor(BoltTheme.electricBlueLight)
                            .rotationEffect(.degrees(engine.uiState.scanning ? 360 : 0))
                            .animation(
                                engine.uiState.scanning ?
                                Animation.linear(duration: 1.0).repeatForever(autoreverses: false) : .default,
                                value: engine.uiState.scanning
                            )
                            .frame(width: 36, height: 36)
                            .background(BoltTheme.bgInput)
                            .clipShape(Circle())
                    }
                }
            }
        }
    }

    // MARK: - 无设备时的雷达扫描空态
    private var emptyRadarState: some View {
        VStack(spacing: 20) {
            Spacer().frame(height: 30)

            RadarView()

            VStack(spacing: 8) {
                Text("正在搜索局域网设备...")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundColor(BoltTheme.textPrimary)

                Text("请确保双方设备接入同一 Wi-Fi 或移动热点，\n且已打开 Bolt 应用。")
                    .font(.system(size: 13))
                    .foregroundColor(BoltTheme.textMuted)
                    .multilineTextAlignment(.center)
                    .lineSpacing(4)
            }

            Button {
                isManualConnectPresented = true
            } label: {
                Text("无法发现？尝试手动输入 IP 直连")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundColor(BoltTheme.cyberTeal)
            }
            .padding(.top, 4)

            Spacer().frame(height: 40)
        }
    }

    // MARK: - 在线设备列表流
    private var devicesList: some View {
        LazyVStack(spacing: 12) {
            ForEach(engine.uiState.devices) { device in
                deviceRow(device: device)
            }
        }
    }

    private func deviceRow(device: DeviceUi) -> some View {
        BtCard {
            VStack(spacing: 12) {
                HStack(spacing: 12) {
                    ZStack {
                        Circle()
                            .fill(BoltTheme.electricBlue.opacity(0.15))
                            .frame(width: 44, height: 44)
                        Image(systemName: device.typeEnum.systemIcon)
                            .font(.system(size: 20))
                            .foregroundColor(BoltTheme.electricBlueLight)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text(device.name)
                            .font(.system(size: 16, weight: .bold))
                            .foregroundColor(BoltTheme.textPrimary)

                        HStack(spacing: 8) {
                            Text("\(device.ip):\(device.quicPort)")
                                .font(.system(size: 12, design: .monospaced))
                                .foregroundColor(BoltTheme.textSecondary)

                            if device.connState == .connected {
                                Text("已连接")
                                    .font(.system(size: 10, weight: .semibold))
                                    .foregroundColor(BoltTheme.emeraldGreen)
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(BoltTheme.emeraldGreen.opacity(0.15))
                                    .cornerRadius(6)
                            }
                        }
                    }

                    Spacer()

                    DeviceBadge(deviceType: device.typeEnum)
                }

                Divider()
                    .background(BoltTheme.borderSubtle)

                // 底部操作栏
                HStack(spacing: 12) {
                    if device.connState == .connected {
                        Button {
                            engine.disconnect(uuid: device.uuid)
                        } label: {
                            Text("断开")
                                .font(.system(size: 13, weight: .medium))
                                .foregroundColor(BoltTheme.textMuted)
                                .frame(maxWidth: .infinity)
                                .frame(height: 38)
                                .background(BoltTheme.bgInput)
                                .cornerRadius(10)
                        }
                    } else {
                        Button {
                            engine.connect(uuid: device.uuid)
                        } label: {
                            Text(device.connState == .connecting ? "连接中..." : "配对连接")
                                .font(.system(size: 13, weight: .medium))
                                .foregroundColor(BoltTheme.electricBlueLight)
                                .frame(maxWidth: .infinity)
                                .frame(height: 38)
                                .background(BoltTheme.bgInput)
                                .cornerRadius(10)
                        }
                        .disabled(device.connState == .connecting)
                    }

                    Button {
                        selectedDeviceForShare = device
                    } label: {
                        HStack(spacing: 6) {
                            Image(systemName: "paperplane.fill")
                                .font(.system(size: 12))
                            Text("发送文件")
                                .font(.system(size: 13, weight: .semibold))
                        }
                        .foregroundColor(.white)
                        .frame(maxWidth: .infinity)
                        .frame(height: 38)
                        .background(BoltTheme.brandGradient)
                        .cornerRadius(10)
                        .shadow(color: BoltTheme.electricBlue.opacity(0.3), radius: 6, x: 0, y: 2)
                    }
                }
            }
        }
    }

    // MARK: - 手动直连弹窗
    private var manualConnectSheet: some View {
        NavigationStack {
            ZStack {
                BoltTheme.bgPrimary.ignoresSafeArea()

                VStack(spacing: 20) {
                    BtCard {
                        VStack(alignment: .leading, spacing: 14) {
                            Text("输入目标设备的局域网 IP 与端口")
                                .font(.system(size: 13))
                                .foregroundColor(BoltTheme.textSecondary)

                            VStack(alignment: .leading, spacing: 6) {
                                Text("IP 地址")
                                    .font(.system(size: 12, weight: .medium))
                                    .foregroundColor(BoltTheme.textMuted)
                                TextField("例如 192.168.1.100", text: $manualIp)
                                    .keyboardType(.numbersAndPunctuation)
                                    .autocorrectionDisabled()
                                    .padding(12)
                                    .background(BoltTheme.bgInput)
                                    .cornerRadius(10)
                                    .foregroundColor(BoltTheme.textPrimary)
                            }

                            VStack(alignment: .leading, spacing: 6) {
                                Text("端口 (默认 8899)")
                                    .font(.system(size: 12, weight: .medium))
                                    .foregroundColor(BoltTheme.textMuted)
                                TextField("8899", text: $manualPort)
                                    .keyboardType(.numberPad)
                                    .padding(12)
                                    .background(BoltTheme.bgInput)
                                    .cornerRadius(10)
                                    .foregroundColor(BoltTheme.textPrimary)
                            }
                        }
                    }

                    Button {
                        let port = UInt16(manualPort) ?? 8899
                        engine.connectAddr(ip: manualIp.trimmingCharacters(in: .whitespaces), port: port)
                        isManualConnectPresented = false
                    } label: {
                        Text("发起连接")
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundColor(.white)
                            .frame(maxWidth: .infinity)
                            .frame(height: 50)
                            .background(BoltTheme.brandGradient)
                            .cornerRadius(14)
                    }
                    .disabled(manualIp.isEmpty)

                    Spacer()
                }
                .padding()
            }
            .navigationTitle("手动直连")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("取消") { isManualConnectPresented = false }
                        .foregroundColor(BoltTheme.textSecondary)
                }
            }
        }
    }
}
