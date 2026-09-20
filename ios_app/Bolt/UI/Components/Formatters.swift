//
//  Formatters.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import Foundation

public struct Formatters {

    /// 格式化字节大小 (如 25.4 MB, 1.20 GB)
    public static func formatBytes(_ bytes: Int64) -> String {
        guard bytes > 0 else { return "0 B" }
        let units = ["B", "KB", "MB", "GB", "TB"]
        var size = Double(bytes)
        var unitIndex = 0
        while size >= 1024.0 && unitIndex < units.count - 1 {
            size /= 1024.0
            unitIndex += 1
        }
        if unitIndex == 0 {
            return "\(Int(size)) B"
        } else {
            return String(format: "%.2f %@", size, units[unitIndex])
        }
    }

    /// 格式化传输瞬时速率 (如 22.4 MB/s)
    public static func formatSpeed(_ bytesPerSec: Int64) -> String {
        guard bytesPerSec > 0 else { return "0 B/s" }
        return "\(formatBytes(bytesPerSec))/s"
    }

    /// 格式化预估剩余时间 (如 01:25)
    public static func formatEta(_ seconds: Int64) -> String {
        guard seconds > 0 else { return "--:--" }
        let mins = seconds / 60
        let secs = seconds % 60
        return String(format: "%02d:%02d", mins, secs)
    }

    /// 格式化毫秒耗时 (如 3.2 秒, 1 分 12 秒)
    public static func formatDuration(ms: Int64) -> String {
        guard ms > 0 else { return "0 秒" }
        let totalSecs = ms / 1000
        if totalSecs < 60 {
            let decimal = Double(ms) / 1000.0
            return String(format: "%.1f 秒", decimal)
        }
        let mins = totalSecs / 60
        let secs = totalSecs % 60
        return "\(mins) 分 \(secs) 秒"
    }

    /// 格式化时间戳
    public static func formatDate(timestampMs: Int64) -> String {
        guard timestampMs > 0 else { return "" }
        let date = Date(timeIntervalSince1970: Double(timestampMs) / 1000.0)
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd HH:mm"
        return formatter.string(from: date)
    }
}
