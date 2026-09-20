//
//  QuickShareSheet.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI
import PhotosUI

/// 发送文件/相册选择与投递半屏模态视图。
public struct QuickShareSheet: View {

    public let targetDevice: DeviceUi
    @Environment(\.dismiss) private var dismiss
    @ObservedObject private var engine = BoltEngine.shared

    @State private var selectedPhotos: [PhotosPickerItem] = []
    @State private var isFileImporterPresented: Bool = false

    public init(targetDevice: DeviceUi) {
        self.targetDevice = targetDevice
    }

    public var body: some View {
        NavigationStack {
            ZStack {
                BoltTheme.bgPrimary.ignoresSafeArea()

                VStack(spacing: 20) {
                    // 目标设备卡片
                    BtCard {
                        HStack(spacing: 12) {
                            ZStack {
                                Circle()
                                    .fill(BoltTheme.electricBlue.opacity(0.15))
                                    .frame(width: 44, height: 44)
                                Image(systemName: targetDevice.typeEnum.systemIcon)
                                    .font(.system(size: 20))
                                    .foregroundColor(BoltTheme.electricBlueLight)
                            }
                            VStack(alignment: .leading, spacing: 4) {
                                Text(targetDevice.name)
                                    .font(.system(size: 16, weight: .semibold))
                                    .foregroundColor(BoltTheme.textPrimary)
                                Text("\(targetDevice.ip):\(targetDevice.quicPort)")
                                    .font(.system(size: 13))
                                    .foregroundColor(BoltTheme.textSecondary)
                            }
                            Spacer()
                            DeviceBadge(deviceType: targetDevice.typeEnum)
                        }
                    }

                    // 选项区：相册 vs 文件
                    VStack(spacing: 14) {
                        // 1. 照片与视频选取
                        PhotosPicker(
                            selection: $selectedPhotos,
                            maxSelectionCount: 50,
                            matching: .any(of: [.images, .videos])
                        ) {
                            HStack(spacing: 16) {
                                ZStack {
                                    RoundedRectangle(cornerRadius: 12)
                                        .fill(Color(hex: "#EC4899").opacity(0.15))
                                        .frame(width: 48, height: 48)
                                    Image(systemName: "photo.stack.fill")
                                        .font(.system(size: 22))
                                        .foregroundColor(Color(hex: "#F472B6"))
                                }
                                VStack(alignment: .leading, spacing: 4) {
                                    Text("从系统相册选取")
                                        .font(.system(size: 15, weight: .semibold))
                                        .foregroundColor(BoltTheme.textPrimary)
                                    Text("高速原图/原视频无损发送")
                                        .font(.system(size: 12))
                                        .foregroundColor(BoltTheme.textSecondary)
                                }
                                Spacer()
                                Image(systemName: "chevron.right")
                                    .foregroundColor(BoltTheme.textMuted)
                            }
                            .padding()
                            .background(BoltTheme.bgCard)
                            .cornerRadius(14)
                            .overlay(RoundedRectangle(cornerRadius: 14).stroke(BoltTheme.borderSubtle, lineWidth: 1))
                        }

                        // 2. 本地文档与文件选取
                        Button {
                            isFileImporterPresented = true
                        } label: {
                            HStack(spacing: 16) {
                                ZStack {
                                    RoundedRectangle(cornerRadius: 12)
                                        .fill(BoltTheme.cyberTeal.opacity(0.15))
                                        .frame(width: 48, height: 48)
                                    Image(systemName: "folder.fill")
                                        .font(.system(size: 22))
                                        .foregroundColor(BoltTheme.cyberTeal)
                                }
                                VStack(alignment: .leading, spacing: 4) {
                                    Text("从文件应用选取")
                                        .font(.system(size: 15, weight: .semibold))
                                        .foregroundColor(BoltTheme.textPrimary)
                                    Text("文档、压缩包、音乐及任意格式")
                                        .font(.system(size: 12))
                                        .foregroundColor(BoltTheme.textSecondary)
                                }
                                Spacer()
                                Image(systemName: "chevron.right")
                                    .foregroundColor(BoltTheme.textMuted)
                            }
                            .padding()
                            .background(BoltTheme.bgCard)
                            .cornerRadius(14)
                            .overlay(RoundedRectangle(cornerRadius: 14).stroke(BoltTheme.borderSubtle, lineWidth: 1))
                        }
                    }

                    Spacer()

                    // 暂存状态提示
                    if engine.uiState.staging {
                        HStack(spacing: 10) {
                            ProgressView()
                                .tint(BoltTheme.cyberTeal)
                            Text("正在暂存准备数据...")
                                .font(.system(size: 14))
                                .foregroundColor(BoltTheme.textSecondary)
                        }
                        .padding(.bottom, 12)
                    }
                }
                .padding()
            }
            .navigationTitle("发送到设备")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("取消") { dismiss() }
                        .foregroundColor(BoltTheme.textSecondary)
                }
            }
            .onChange(of: selectedPhotos, perform: { newItems in
                guard !newItems.isEmpty else { return }
                Task {
                    await engine.sendPhotos(targetUuid: targetDevice.uuid, items: newItems)
                    dismiss()
                }
            })
            .fileImporter(
                isPresented: $isFileImporterPresented,
                allowedContentTypes: [.item],
                allowsMultipleSelection: true
            ) { result in
                switch result {
                case .success(let urls):
                    guard !urls.isEmpty else { return }
                    Task {
                        await engine.sendFiles(targetUuid: targetDevice.uuid, urls: urls)
                        dismiss()
                    }
                case .failure(let error):
                    engine.toastMessage = "文件选择异常: \(error.localizedDescription)"
                }
            }
        }
    }
}
