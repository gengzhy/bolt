//
//  BoltTheme.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

/// Bolt 全局视觉设计规范与主题色彩。
public struct BoltTheme {

    // 背景色系
    public static let bgDeep = Color(hex: "#090D16")
    public static let bgPrimary = Color(hex: "#0F172A")
    public static let bgCard = Color(hex: "#1E293B")
    public static let bgCardHover = Color(hex: "#27354E")
    public static let bgInput = Color(hex: "#141D2E")

    // 品牌强调色
    public static let electricBlue = Color(hex: "#2563EB")
    public static let electricBlueLight = Color(hex: "#3B82F6")
    public static let cyberTeal = Color(hex: "#06B6D4")
    public static let emeraldGreen = Color(hex: "#10B981")
    public static let amberWarning = Color(hex: "#F59E0B")
    public static let roseError = Color(hex: "#F43F5E")

    // 文本色系
    public static let textPrimary = Color.white
    public static let textSecondary = Color(hex: "#94A3B8")
    public static let textMuted = Color(hex: "#64748B")

    // 边框与阴影
    public static let borderSubtle = Color.white.opacity(0.08)
    public static let borderHighlight = Color.white.opacity(0.18)

    // 渐变色
    public static let brandGradient = LinearGradient(
        colors: [electricBlue, cyberTeal],
        startPoint: .topLeading,
        endPoint: .bottomTrailing
    )

    public static let cardGradient = LinearGradient(
        colors: [bgCard, bgCard.opacity(0.85)],
        startPoint: .top,
        endPoint: .bottom
    )
}

// MARK: - 辅助 Hex 颜色初始化
extension Color {
    init(hex: String) {
        let hex = hex.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        var int: UInt64 = 0
        Scanner(string: hex).scanHexInt64(&int)
        let a, r, g, b: UInt64
        switch hex.count {
        case 3: // RGB (12-bit)
            (a, r, g, b) = (255, (int >> 8) * 17, (int >> 4 & 0xF) * 17, (int & 0xF) * 17)
        case 6: // RGB (24-bit)
            (a, r, g, b) = (255, int >> 16, int >> 8 & 0xFF, int & 0xFF)
        case 8: // ARGB (32-bit)
            (a, r, g, b) = (int >> 24, int >> 16 & 0xFF, int >> 8 & 0xFF, int & 0xFF)
        default:
            (a, r, g, b) = (1, 1, 1, 0)
        }
        self.init(
            .sRGB,
            red: Double(r) / 255,
            green: Double(g) / 255,
            blue: Double(b) / 255,
            opacity: Double(a) / 255
        )
    }
}
