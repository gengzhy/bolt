//! 文件身份标识：相对路径 + 文件大小 + 修改时间（实施方案 4.3-3）。
//!
//! 断点续传时用于匹配已收区间；不匹配则判定源文件已变更，需全量重传。

use serde::{Deserialize, Serialize};
use std::path::Path;

use lt_utils::{LtError, LtResult};

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
    pub fn from_fs(abs_path: &Path, rel_path: &str) -> LtResult<FileIdentity> {
        let meta = std::fs::metadata(abs_path).map_err(|_| LtError::FileNotAccessible)?;
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

    /// 是否与另一个身份匹配（断点续传协商用）。
    /// 当相对路径、文件大小和修改时间完全一致时，判定为同一文件可进行断点恢复。
    pub fn matches(&self, other: &FileIdentity) -> bool {
        self.rel_path == other.rel_path
            && self.size == other.size
            && self.mtime_unix == other.mtime_unix
    }

    /// 断点缓存键（内容寻址）。
    /// 基于「相对路径|文件大小|修改时间」计算 BLAKE3 64字符十六进制摘要。
    pub fn cache_key(&self) -> String {
        let raw = format!("{}|{}|{}", self.rel_path, self.size, self.mtime_unix);
        blake3::hash(raw.as_bytes()).to_hex().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_semantics() {
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
        assert!(a.matches(&b));
        let c = FileIdentity {
            size: 11,
            ..a.clone()
        };
        assert!(!a.matches(&c));
        assert_eq!(a.cache_key().len(), 64);
    }
}
