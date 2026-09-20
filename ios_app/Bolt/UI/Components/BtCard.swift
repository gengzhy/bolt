//
//  BtCard.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

/// 统一暗黑玻璃拟态卡片容器组件。
public struct BtCard<Content: View>: View {

    private let content: Content
    private let padding: CGFloat

    public init(padding: CGFloat = 16, @ViewBuilder content: () -> Content) {
        self.padding = padding
        self.content = content()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            content
        }
        .padding(padding)
        .background(
            RoundedRectangle(cornerRadius: 16)
                .fill(BoltTheme.bgCard)
                .overlay(
                    RoundedRectangle(cornerRadius: 16)
                        .stroke(BoltTheme.borderSubtle, lineWidth: 1)
                )
        )
        .shadow(color: Color.black.opacity(0.25), radius: 8, x: 0, y: 4)
    }
}
