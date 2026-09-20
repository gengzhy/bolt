//
//  StagingService.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import Foundation
import PhotosUI
import SwiftUI

/// 文件与相册暂存服务。
///
/// 背景：
/// 1. iOS 安全作用域 URL (Security-Scoped URLs) 在视图关闭后权限可能失效；
/// 2. 相册选择的图片/视频在系统相册私有存储中，Rust 核心需要标准 POSIX 文件路径；
/// 3. 因此在发送前，将用户选中的文件平铺暂存至沙盒 `Caches/outbox/<UUID>/` 目录，传给 `bt_send_files`。
public final class StagingService {

    public static let shared = StagingService()

    private let fileManager = FileManager.default

    private init() {}

    /// 暂存根目录
    private var outboxRoot: URL {
        let caches = fileManager.urls(for: .cachesDirectory, in: .userDomainMask)[0]
        let outbox = caches.appendingPathComponent("outbox", isDirectory: true)
        try? fileManager.createDirectory(at: outbox, withIntermediateDirectories: true)
        return outbox
    }

    // =========================================================================
    // 文档与本地文件暂存
    // =========================================================================

    /// 暂存多个选中的文件 URL（支持安全作用域）
    /// - Parameter urls: 外部文件 URL 列表
    /// - Returns: (暂存根目录, 绝对路径数组, 总字节数)
    public func stageFiles(urls: [URL]) async throws -> (stagingDir: URL, paths: [String], totalSize: Int64) {
        let taskDir = outboxRoot.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try fileManager.createDirectory(at: taskDir, withIntermediateDirectories: true)

        var stagedPaths: [String] = []
        var totalBytes: Int64 = 0

        for url in urls {
            let accessing = url.startAccessingSecurityScopedResource()
            defer {
                if accessing {
                    url.stopAccessingSecurityScopedResource()
                }
            }

            let filename = url.lastPathComponent
            let dest = taskDir.appendingPathComponent(filename)

            // 若同名则追加序号
            let uniqueDest = makeUniqueDest(taskDir: taskDir, filename: filename)

            try fileManager.copyItem(at: url, to: uniqueDest)
            stagedPaths.append(uniqueDest.path)

            if let attrs = try? fileManager.attributesOfItem(atPath: uniqueDest.path),
               let size = attrs[.size] as? Int64 {
                totalBytes += size
            }
        }

        return (taskDir, stagedPaths, totalBytes)
    }

    // =========================================================================
    // 系统相册选择暂存 (PhotosPickerItem)
    // =========================================================================

    /// 暂存从 PhotosPicker 选中的图片与视频
    public func stagePhotosItems(items: [PhotosPickerItem]) async throws -> (stagingDir: URL, paths: [String], totalSize: Int64) {
        let taskDir = outboxRoot.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try fileManager.createDirectory(at: taskDir, withIntermediateDirectories: true)

        var stagedPaths: [String] = []
        var totalBytes: Int64 = 0

        for (index, item) in items.enumerated() {
            // 1. 尝试作为视频/原文件载入
            if let movie = try? await item.loadTransferable(type: MovieFileTransferable.self) {
                let filename = "video_\(index + 1).\(movie.url.pathExtension.isEmpty ? "mp4" : movie.url.pathExtension)"
                let dest = taskDir.appendingPathComponent(filename)
                try? fileManager.copyItem(at: movie.url, to: dest)
                stagedPaths.append(dest.path)
                if let attrs = try? fileManager.attributesOfItem(atPath: dest.path),
                   let size = attrs[.size] as? Int64 {
                    totalBytes += size
                }
                continue
            }

            // 2. 尝试作为图片数据载入
            if let data = try? await item.loadTransferable(type: Data.self) {
                // 根据数据头简要判定格式
                let ext = data.isPNG ? "png" : "jpg"
                let filename = "photo_\(index + 1).\(ext)"
                let dest = taskDir.appendingPathComponent(filename)
                try data.write(to: dest)
                stagedPaths.append(dest.path)
                totalBytes += Int64(data.count)
            }
        }

        return (taskDir, stagedPaths, totalBytes)
    }

    // =========================================================================
    // 清理与垃圾回收
    // =========================================================================

    /// 清理单个任务的暂存目录
    public func cleanup(stagingDir: URL) {
        try? fileManager.removeItem(at: stagingDir)
    }

    /// 定期扫描清理超过 24 小时的孤立暂存目录
    public func sweepStaleOutbox() {
        guard let contents = try? fileManager.contentsOfDirectory(at: outboxRoot, includingPropertiesForKeys: [.contentModificationDateKey]) else { return }
        let now = Date()
        for dir in contents {
            if let date = try? dir.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate,
               now.timeIntervalSince(date) > 86400 {
                try? fileManager.removeItem(at: dir)
            }
        }
    }

    private func makeUniqueDest(taskDir: URL, filename: String) -> URL {
        var dest = taskDir.appendingPathComponent(filename)
        if !fileManager.fileExists(atPath: dest.path) {
            return dest
        }
        let baseName = (filename as NSString).deletingPathExtension
        let ext = (filename as NSString).pathExtension
        var counter = 1
        while fileManager.fileExists(atPath: dest.path) {
            let newName = ext.isEmpty ? "\(baseName)_\(counter)" : "\(baseName)_\(counter).\(ext)"
            dest = taskDir.appendingPathComponent(newName)
            counter += 1
        }
        return dest
    }
}

// MARK: - 辅助 Transferable 支持读取视频文件
struct MovieFileTransferable: Transferable {
    let url: URL

    static var transferRepresentation: some TransferRepresentation {
        FileRepresentation(importedContentType: .movie) { received in
            let copyUrl = FileManager.default.temporaryDirectory.appendingPathComponent(received.file.lastPathComponent)
            try? FileManager.default.removeItem(at: copyUrl)
            try FileManager.default.copyItem(at: received.file, to: copyUrl)
            return Self(url: copyUrl)
        }
    }
}

private extension Data {
    var isPNG: Bool {
        guard self.count >= 8 else { return false }
        let pngHeader: [UInt8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        return Array(self.prefix(8)) == pngHeader
    }
}
