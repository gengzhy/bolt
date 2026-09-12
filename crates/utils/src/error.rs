//! 统一错误码（实施方案第十二节）。
//!
//! 规则：底层异常全部转换为错误码，不崩溃进程；UI 仅按错误码展示文案。

use thiserror::Error;

/// Bolt 统一错误码枚举。数值与文档一一对应。
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum BtError {
    /// -1 参数非法
    #[error("参数非法")]
    InvalidArgument,
    /// -2 端口被占用 / 端口池耗尽
    #[error("端口被占用或端口池耗尽")]
    PortUnavailable,
    /// -3 连接超时 / 远端离线
    #[error("连接超时或远端离线")]
    ConnectTimeout,
    /// -4 文件不存在或无读取权限
    #[error("文件不存在或无读取权限")]
    FileNotAccessible,
    /// -5 磁盘剩余空间不足
    #[error("磁盘剩余空间不足")]
    DiskFull,
    /// -6 文件校验失败（BLAKE3 不匹配）
    #[error("文件校验失败")]
    ChecksumMismatch,
    /// -7 发现服务不可用（组播被拦截，已提示用户尝试兜底通道）
    #[error("发现服务不可用")]
    DiscoveryUnavailable,
    /// -8 内存映射文件失败
    #[error("内存映射文件失败")]
    MmapFailed,
    /// -9 任务已被用户取消
    #[error("任务已被用户取消")]
    Cancelled,
    /// -10 系统权限不足（文件/网络/通知）
    #[error("系统权限不足")]
    PermissionDenied,
    /// -11 配对失败（对方拒绝或验证码不一致）
    #[error("配对失败")]
    PairingFailed,
    /// -12 传输被对方拒绝
    #[error("传输被对方拒绝")]
    TransferRejected,
    /// -13 协议版本不兼容
    #[error("协议版本不兼容")]
    ProtocolIncompatible,
    /// -14 对端证书指纹变更（疑似中间人，连接已阻止）
    #[error("对端证书指纹变更，疑似中间人")]
    FingerprintChanged,
    /// -15 内部错误（详见本地日志）
    #[error("内部错误")]
    Internal,
    /// 其他未分类错误（对外仍映射为 -15）
    #[error("{0}")]
    Other(String),
}

impl BtError {
    /// 错误码数值（0 表示成功，错误均为负值）。
    #[inline]
    pub fn code(&self) -> i32 {
        match self {
            BtError::InvalidArgument => -1,
            BtError::PortUnavailable => -2,
            BtError::ConnectTimeout => -3,
            BtError::FileNotAccessible => -4,
            BtError::DiskFull => -5,
            BtError::ChecksumMismatch => -6,
            BtError::DiscoveryUnavailable => -7,
            BtError::MmapFailed => -8,
            BtError::Cancelled => -9,
            BtError::PermissionDenied => -10,
            BtError::PairingFailed => -11,
            BtError::TransferRejected => -12,
            BtError::ProtocolIncompatible => -13,
            BtError::FingerprintChanged => -14,
            BtError::Internal => -15,
            BtError::Other(_) => -15,
        }
    }

    /// 由数值还原错误码（未知数值归入内部错误）。
    #[inline]
    pub fn from_code(code: i32) -> BtError {
        match code {
            -1 => BtError::InvalidArgument,
            -2 => BtError::PortUnavailable,
            -3 => BtError::ConnectTimeout,
            -4 => BtError::FileNotAccessible,
            -5 => BtError::DiskFull,
            -6 => BtError::ChecksumMismatch,
            -7 => BtError::DiscoveryUnavailable,
            -8 => BtError::MmapFailed,
            -9 => BtError::Cancelled,
            -10 => BtError::PermissionDenied,
            -11 => BtError::PairingFailed,
            -12 => BtError::TransferRejected,
            -13 => BtError::ProtocolIncompatible,
            -14 => BtError::FingerprintChanged,
            -15 => BtError::Internal,
            0 => BtError::Other("success".into()),
            _ => BtError::Internal,
        }
    }
}

/// 统一结果类型。
pub type BtResult<T> = Result<T, BtError>;

impl From<std::io::Error> for BtError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => BtError::FileNotAccessible,
            std::io::ErrorKind::PermissionDenied => BtError::PermissionDenied,
            std::io::ErrorKind::AddrInUse => BtError::PortUnavailable,
            std::io::ErrorKind::TimedOut => BtError::ConnectTimeout,
            _ => {
                tracing::debug!(error = %e, "io error mapped to Internal");
                BtError::Internal
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_roundtrip() {
        for code in -15..=-1 {
            let err = BtError::from_code(code);
            assert_eq!(err.code(), code, "code {code} roundtrip failed");
        }
    }

    #[test]
    fn known_codes() {
        assert_eq!(BtError::ChecksumMismatch.code(), -6);
        assert_eq!(BtError::FingerprintChanged.code(), -14);
        assert_eq!(BtError::Other("x".into()).code(), -15);
    }
}
