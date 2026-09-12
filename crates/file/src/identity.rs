//! 文件身份标识：相对路径 + 文件大小 + 修改时间（实施方案 4.3-3）。
//!
//! 断点续传时用于匹配已收区间；不匹配则判定源文件已变更，需全量重传。

use serde::{Deserialize, Serialize};
use std::path::Path;

use utils::{BtError, BtResult};

/// 唯一标识一个待传输文件。
///
/// 【设计说明】：使用「相对路径 + 文件字节大小 + 修改时间戳 (mtime)」作为轻量级
/// 且跨端完全兼容的文件三元组身份。
/// 避免在遍历成千上万个文件时同步读取磁盘做前缀哈希，从而杜绝磁盘 I/O 拥塞与协议破坏。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileIdentity {
    /// 相对路径（统一 '/' 分隔符）
    pub rel_path: String,
    /// 文件字节数
    pub size: u64,
    /// 修改时间（unix 秒）
    pub mtime_unix: i64,
}

impl FileIdentity {
    /// 从文件系统元数据构造。
    /// 只读取文件元信息（fs::metadata），避免任何阻塞式读取磁盘文件内容。
    pub fn from_fs(abs_path: &Path, rel_path: &str) -> BtResult<FileIdentity> {
        let meta = std::fs::metadata(abs_path).map_err(|_| BtError::FileNotAccessible)?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        Ok(FileIdentity {
            rel_path: rel_path.to_string(),
            size: meta.len(),
            mtime_unix: mtime,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_fields() {
        let a = FileIdentity {
            rel_path: "a/b.txt".into(),
            size: 10,
            mtime_unix: 123,
        };
        let b = FileIdentity {
            rel_path: "a/b.txt".into(),
            size: 10,
            mtime_unix: 123,
        };
        assert_eq!(a, b);
        let c = FileIdentity {
            size: 11,
            ..a.clone()
        };
        assert_ne!(a, c);
    }
}
