//
//  ProgressBar.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

/// 极速传输渐变进度条组件。
public struct ProgressBar: View {

    public let progress: Double
    public let height: CGFloat
    public let showGlow: Bool

    public init(progress: Double, height: CGFloat = 8, showGlow: Bool = true) {
        self.progress = min(1.0, max(0.0, progress))
        self.height = height
        self.showGlow = showGlow
    }

    public var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                // 槽底
                Capsule()
                    .fill(Color.white.opacity(0.08))
                    .frame(height: height)

                // 进度填充
                Capsule()
                    .fill(
                        LinearGradient(
                            colors: [BoltTheme.electricBlue, BoltTheme.cyberTeal],
                            startPoint: .leading,
                            endPoint: .trailing
                        )
                    )
                    .frame(width: max(geometry.size.width * CGFloat(progress), height), height: height)
                    .shadow(
                        color: showGlow ? BoltTheme.cyberTeal.opacity(0.4) : Color.clear,
                        radius: 6,
                        x: 0,
                        y: 0
                    )
                    .animation(.spring(response: 0.35, dampingFraction: 0.8), value: progress)
            }
        }
        .frame(height: height)
    }
}
