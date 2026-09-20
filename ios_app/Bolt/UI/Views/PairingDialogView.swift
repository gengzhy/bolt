//
//  PairingDialogView.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

/// 4 位验证码屏幕比对与配对确认弹窗。
public struct PairingDialogView: View {

    public let pairId: UInt64
    public let uuid: String
    public let name: String
    public let code: String

    @ObservedObject private var engine = BoltEngine.shared

    public init(pairId: UInt64, uuid: String, name: String, code: String) {
        self.pairId = pairId
        self.uuid = uuid
        self.name = name
        self.code = code
    }

    public var body: some View {
        ZStack {
            BoltTheme.bgPrimary.ignoresSafeArea()

            VStack(spacing: 24) {
                // 图标
                ZStack {
                    Circle()
                        .fill(BoltTheme.electricBlue.opacity(0.15))
                        .frame(width: 72, height: 72)
                    Image(systemName: "lock.shield.fill")
                        .font(.system(size: 36))
                        .foregroundColor(BoltTheme.electricBlueLight)
                }
                .padding(.top, 16)

                VStack(spacing: 8) {
                    Text("安全配对请求")
                        .font(.system(size: 20, weight: .bold))
                        .foregroundColor(BoltTheme.textPrimary)

                    Text("来自设备「\(name)」的发起请求")
                        .font(.system(size: 14))
                        .foregroundColor(BoltTheme.textSecondary)
                }

                // 4 位验证码展示卡片
                HStack(spacing: 12) {
                    ForEach(Array(code.enumerated()), id: \.offset) { _, char in
                        Text(String(char))
                            .font(.system(size: 32, weight: .heavy, design: .monospaced))
                            .foregroundColor(BoltTheme.cyberTeal)
                            .frame(width: 56, height: 68)
                            .background(BoltTheme.bgCard)
                            .cornerRadius(12)
                            .overlay(
                                RoundedRectangle(cornerRadius: 12)
                                    .stroke(BoltTheme.cyberTeal.opacity(0.4), lineWidth: 1.5)
                            )
                    }
                }
                .padding(.vertical, 8)

                Text("请核对双方屏幕上显示的 4 位验证码是否一致。\n确认一致后对方将被加入受信任设备白名单。")
                    .font(.system(size: 13))
                    .foregroundColor(BoltTheme.textMuted)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)

                Spacer()

                // 操作按钮
                HStack(spacing: 16) {
                    Button {
                        engine.respondPair(pairId: pairId, accept: false)
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
                        engine.respondPair(pairId: pairId, accept: true)
                    } label: {
                        Text("确认匹配")
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundColor(.white)
                            .frame(maxWidth: .infinity)
                            .frame(height: 50)
                            .background(BoltTheme.brandGradient)
                            .cornerRadius(14)
                            .shadow(color: BoltTheme.electricBlue.opacity(0.4), radius: 8, x: 0, y: 4)
                    }
                }
                .padding(.horizontal)
                .padding(.bottom, 16)
            }
            .padding()
        }
    }
}
