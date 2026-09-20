//
//  RadarView.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

/// 局域网设备扫描动态雷达波纹视图。
public struct RadarView: View {

    @State private var isAnimating: Bool = false

    public init() {}

    public var body: some View {
        ZStack {
            // 外圈波纹 3
            Circle()
                .stroke(BoltTheme.cyberTeal.opacity(0.15), lineWidth: 1.5)
                .scaleEffect(isAnimating ? 1.6 : 0.8)
                .opacity(isAnimating ? 0.0 : 0.6)

            // 外圈波纹 2
            Circle()
                .stroke(BoltTheme.electricBlue.opacity(0.25), lineWidth: 2)
                .scaleEffect(isAnimating ? 1.3 : 0.6)
                .opacity(isAnimating ? 0.0 : 0.8)

            // 核心辐射圈
            Circle()
                .fill(
                    RadialGradient(
                        gradient: Gradient(colors: [
                            BoltTheme.electricBlue.opacity(0.35),
                            BoltTheme.cyberTeal.opacity(0.05),
                            Color.clear
                        ]),
                        center: .center,
                        startRadius: 5,
                        endRadius: 50
                    )
                )
                .frame(width: 100, height: 100)

            // 核心雷达图标
            ZStack {
                Circle()
                    .fill(BoltTheme.brandGradient)
                    .frame(width: 52, height: 52)
                    .shadow(color: BoltTheme.electricBlue.opacity(0.5), radius: 10)

                Image(systemName: "bolt.fill")
                    .font(.system(size: 24, weight: .bold))
                    .foregroundColor(.white)
            }
        }
        .frame(width: 140, height: 140)
        .onAppear {
            withAnimation(
                Animation.easeOut(duration: 2.2)
                    .repeatForever(autoreverses: false)
            ) {
                isAnimating = true
            }
        }
    }
}
