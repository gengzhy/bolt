//
//  DeviceBadge.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import SwiftUI

/// 设备操作系统徽标胶囊。
public struct DeviceBadge: View {

    public let deviceType: DeviceType

    public init(deviceType: DeviceType) {
        self.deviceType = deviceType
    }

    public var body: some View {
        HStack(spacing: 5) {
            Image(systemName: deviceType.systemIcon)
                .font(.system(size: 11, weight: .semibold))
            Text(deviceType.title)
                .font(.system(size: 11, weight: .medium))
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(badgeBackground)
        .foregroundColor(badgeForeground)
        .clipShape(Capsule())
    }

    private var badgeBackground: Color {
        switch deviceType {
        case .windows: return Color(hex: "#0078D4").opacity(0.18)
        case .android: return Color(hex: "#3DDC84").opacity(0.18)
        case .ios, .macos: return BoltTheme.electricBlue.opacity(0.18)
        case .linux: return Color(hex: "#FCC624").opacity(0.18)
        case .unknown: return Color.white.opacity(0.1)
        }
    }

    private var badgeForeground: Color {
        switch deviceType {
        case .windows: return Color(hex: "#60A5FA")
        case .android: return Color(hex: "#34D399")
        case .ios, .macos: return Color(hex: "#93C5FD")
        case .linux: return Color(hex: "#FDE047")
        case .unknown: return BoltTheme.textSecondary
        }
    }
}
