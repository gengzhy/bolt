//! 文件身份标识：相对路径 + 文件大小 + 修改时间（实施方案 4.3-3）。
//!
//! 断点续传时用于匹配已收区间；不匹配则判定源文件已变更，需全量重传。

use serde::{Deserialize, Serialize};
use std::path::Path;

use lt_utils::{LtError, LtResult};

/// 唯一标识一个待传输文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileIdentity {
    /// 相对路径（统一 '/' 分隔符）
    pub rel_path: String,
    /// 文件字节数
    pub size: u64,
    /// 修改时间（unix 秒）
    pub mtime_unix: i64,
    /// 头部数据的 BLAKE3 哈希（前 16 字节），用于防静默篡改
    pub head_hash: [u8; 16],
}

impl FileIdentity {
    /// 从文件系统元数据构造。
    pub fn from_fs(abs_path: &Path, rel_path: &str) -> LtResult<FileIdentity> {
        let meta = std::fs::metadata(abs_path).map_err(|_| LtError::FileNotAccessible)?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        
        let mut head_hash = [0u8; 16];
        if meta.len() > 0 {
            if let Ok(mut f) = std::fs::File::open(abs_path) {
                use std::io::Read;
                let mut buf = [0u8; 8192];
                if let Ok(n) = f.read(&mut buf) {
                    let full_hash = blake3::hash(&buf[..n]);
                    head_hash.copy_from_slice(&full_hash.as_bytes()[..16]);
                }
            }
        }

        Ok(FileIdentity {
            rel_path: rel_path.to_string(),
            size: meta.len(),
            mtime_unix: mtime,
            head_hash,
        })
    }

    /// 是否与另一个身份匹配（断点续传协商用）。
    pub fn matches(&self, other: &FileIdentity) -> bool {
        self.rel_path == other.rel_path
            && self.size == other.size
            && self.mtime_unix == other.mtime_unix
            && self.head_hash == other.head_hash
    }

    /// 断点缓存键（内容寻址）。
    pub fn cache_key(&self) -> String {
        let head_hex = self.head_hash.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        let raw = format!("{}|{}|{}|{}", self.rel_path, self.size, self.mtime_unix, head_hex);
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
            head_hash: [0; 16],
        };
        let b = FileIdentity {
            rel_path: "a/b.txt".into(),
            size: 10,
            mtime_unix: 123,
            head_hash: [0; 16],
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
