//
//  TransferRequestDialogView.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

/// 入站文件传输询问弹窗。
public struct TransferRequestDialogView: View {

    public let reqId: UInt64
    public let uuid: String
    public let name: String
    public let fileCount: Int
    public let totalSize: Int64

    @ObservedObject private var engine = BoltEngine.shared

    public init(reqId: UInt64, uuid: String, name: String, fileCount: Int, totalSize: Int64) {
        self.reqId = reqId
        self.uuid = uuid
        self.name = name
        self.fileCount = fileCount
        self.totalSize = totalSize
    }

    public var body: some View {
        ZStack {
            BoltTheme.bgPrimary.ignoresSafeArea()

            VStack(spacing: 24) {
                // 传输图标
                ZStack {
                    Circle()
                        .fill(BoltTheme.cyberTeal.opacity(0.15))
                        .frame(width: 72, height: 72)
                    Image(systemName: "arrow.down.circle.fill")
                        .font(.system(size: 38))
                        .foregroundColor(BoltTheme.cyberTeal)
                }
                .padding(.top, 16)

                VStack(spacing: 8) {
                    Text("收到文件传输请求")
                        .font(.system(size: 20, weight: .bold))
                        .foregroundColor(BoltTheme.textPrimary)

                    Text("来自设备「\(name)」")
                        .font(.system(size: 14))
                        .foregroundColor(BoltTheme.textSecondary)
                }

                // 传输元数据详情卡片
                BtCard {
                    HStack(spacing: 20) {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("文件数量")
                                .font(.system(size: 12))
                                .foregroundColor(BoltTheme.textMuted)
                            Text("\(fileCount) 项")
                                .font(.system(size: 18, weight: .bold))
                                .foregroundColor(BoltTheme.textPrimary)
                        }

                        Divider()
                            .background(BoltTheme.borderSubtle)
                            .frame(height: 36)

                        VStack(alignment: .leading, spacing: 6) {
                            Text("总数据量")
                                .font(.system(size: 12))
                                .foregroundColor(BoltTheme.textMuted)
                            Text(Formatters.formatBytes(totalSize))
                                .font(.system(size: 18, weight: .bold))
                                .foregroundColor(BoltTheme.cyberTeal)
                        }
                    }
                    .frame(maxWidth: .infinity)
                }

                Text("接收后文件将自动保存至「文件」应用中的 Bolt 专区，您可随时查看、导出或存入相册。")
                    .font(.system(size: 13))
                    .foregroundColor(BoltTheme.textMuted)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)

                Spacer()

                // 操作按钮
                HStack(spacing: 16) {
                    Button {
                        engine.respondTransfer(reqId: reqId, accept: false)
                    } label: {
                        Text("拒绝")
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundColor(BoltTheme.roseError)
                            .frame(maxWidth: .infinity)
                            .frame(height: 50)
                            .background(BoltTheme.bgCard)
                            .cornerRadius(14)
                            .overlay(RoundedRectangle(cornerRadius: 14).stroke(BoltTheme.roseError.opacity(0.3), lineWidth: 1))
                    }

                    Button {
                        engine.respondTransfer(reqId: reqId, accept: true)
                    } label: {
                        Text("同意接收")
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundColor(.white)
                            .frame(maxWidth: .infinity)
                            .frame(height: 50)
                            .background(BoltTheme.brandGradient)
                            .cornerRadius(14)
                            .shadow(color: BoltTheme.cyberTeal.opacity(0.4), radius: 8, x: 0, y: 4)
                    }
                }
                .padding(.horizontal)
                .padding(.bottom, 16)
            }
            .padding()
        }
    }
}
