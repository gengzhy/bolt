//! 断点缓存持久化（实施方案 7.2/8.2）。
//!
//! 接收端按文件身份键持久化已收区间；
//! 重连后在 FILE_META_ACK 回传，发送端跳过已完成分片续传。

use std::fs;
use std::path::{Path, PathBuf};

use lt_utils::{LtError, LtResult};

use crate::ranges::RangeSet;

/// 断点缓存目录管理器（`data_dir/resume/<key>.json`）。
pub struct ResumeStore {
    dir: PathBuf,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Entry {
    rel_path: String,
    size: u64,
    ranges: Vec<(u64, u64)>,
    updated_unix: u64,
}

impl ResumeStore {
    pub fn new(data_dir: &Path) -> ResumeStore {
        ResumeStore {
            dir: data_dir.join("resume"),
        }
    }

    fn path_for(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }

    /// 保存某文件的已收区间。
    pub fn save(&self, key: &str, rel_path: &str, size: u64, ranges: &RangeSet) -> LtResult<()> {
        fs::create_dir_all(&self.dir).map_err(|_| LtError::Internal)?;
        let entry = Entry {
            rel_path: rel_path.to_string(),
            size,
            ranges: ranges.intervals().to_vec(),
            updated_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };
        let s = serde_json::to_string(&entry).map_err(|_| LtError::Internal)?;
        fs::write(self.path_for(key), s).map_err(|_| LtError::Internal)
    }

    /// 读取已收区间（不存在返回 None）。
    pub fn load(&self, key: &str) -> Option<RangeSet> {
        let s = fs::read_to_string(self.path_for(key)).ok()?;
        let entry: Entry = serde_json::from_str(&s).ok()?;
        Some(RangeSet::from_intervals(entry.ranges))
    }

    /// 删除一条记录（文件完成/任务取消后）。
    pub fn remove(&self, key: &str) {
        let _ = fs::remove_file(self.path_for(key));
    }

    /// 清空全部断点缓存（设置页"清临时缓存"）。
    pub fn clear_all(&self) -> LtResult<usize> {
        if !self.dir.exists() {
            return Ok(0);
        }
        let mut count = 0;
        for entry in fs::read_dir(&self.dir)
            .map_err(|_| LtError::Internal)?
            .flatten()
        {
            if entry
                .path()
                .extension()
                .map(|e| e == "json")
                .unwrap_or(false)
                && fs::remove_file(entry.path()).is_ok()
            {
                count += 1;
            }
        }
        Ok(count)
    }

    /// 清理过期的僵尸断点缓存和临时目录 (TTL & Garbage Collection)
    /// `max_days` 限制未活动的最高天数
    pub fn cleanup_stale_cache(&self, tmp_dir: &Path, max_days: u64) {
        let max_age = std::time::Duration::from_secs(max_days * 24 * 3600);
        let now = std::time::SystemTime::now();

        // 1. 清理过期 JSON
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        if now.duration_since(mtime).unwrap_or_default() > max_age {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }

        // 2. 清理过期的临时任务文件夹
        if let Ok(entries) = fs::read_dir(tmp_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("task_") {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(mtime) = meta.modified() {
                            if now.duration_since(mtime).unwrap_or_default() > max_age {
                                let _ = fs::remove_dir_all(entry.path());
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_remove() {
        let base = std::env::temp_dir().join(format!("lt-resume-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let store = ResumeStore::new(&base);

        let mut r = RangeSet::new();
        r.insert(0, 100);
        r.insert(200, 300);
        store.save("key1", "a/b.bin", 500, &r).unwrap();

        let loaded = store.load("key1").unwrap();
        assert_eq!(loaded.intervals(), &[(0, 100), (200, 300)]);
        assert!(store.load("missing").is_none());

        store.remove("key1");
        assert!(store.load("key1").is_none());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn clear_all() {
        let base = std::env::temp_dir().join(format!("lt-resume2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let store = ResumeStore::new(&base);
        let r = RangeSet::new();
        store.save("k1", "x", 1, &r).unwrap();
        store.save("k2", "y", 2, &r).unwrap();
        assert_eq!(store.clear_all().unwrap(), 2);
        let _ = fs::remove_dir_all(&base);
    }
}
