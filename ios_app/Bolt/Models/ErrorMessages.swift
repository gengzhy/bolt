//
//  ErrorMessages.swift
//  Bolt
//
//  Created for Bolt iOS Native Engine.
//

import Foundation

/// 统一错误码 → 本地化中文文案（与 Rust BtError 及 Android 端 ErrorMessages 严格对齐）。
public struct ErrorMessages {

    public static func of(_ code: Int, fallback: String? = nil) -> String {
        switch code {
        case 0:
            return "成功"
        case -1:
            return "参数非法"
        case -2:
            return "网络端口被占用"
        case -3:
            return "连接超时或远端设备离线"
        case -4:
            return "文件不存在或无读取权限"
        case -5:
            return "磁盘剩余空间不足"
        case -6:
            return "文件校验失败（内容损坏），请重新传输"
        case -7:
            return "设备发现不可用（组播可能被拦截，可尝试手动输入 IP 连接）"
        case -8:
            return "文件读取失败（内存映射错误）"
        case -9:
            return "任务已被取消"
        case -10:
            return "系统权限不足（文件/网络/通知）"
        case -11:
            return "配对失败（对方拒绝或验证码不一致）"
        case -12:
            return "传输被对方拒绝"
        case -13:
            return "协议版本不兼容，请升级两端应用"
        case -14:
            return "对端证书指纹变更（疑似中间人），连接已阻止"
        case -15:
            return "内部错误（详见本地日志）"
        default:
            return fallback ?? "未知错误 (\(code))"
        }
    }
}
