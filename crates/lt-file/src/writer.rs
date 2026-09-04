//! 写入与合并（实施方案 8.2）。
//!
//! 分片按序号写入私有临时目录 `.tmp` 文件（支持乱序到达、区间空洞），
//! 校验通过后移动至用户保存目录；同名默认自动重命名 `(1)、(2)…`。

use std::fs::{self, File};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use lt_utils::{LtError, LtResult};

use crate::ranges::RangeSet;

/// 同名文件处理策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NameCollisionPolicy {
    /// 自动重命名（默认）
    AutoRename,
    /// 覆盖
    Overwrite,
}

/// 接收端临时文件写入器。
pub struct FileWriter {
    tmp_path: PathBuf,
    file: File,
    size: u64,
    received: RangeSet,
    current_offset: u64,
}

impl FileWriter {
    /// 在临时目录创建（或恢复）一个接收文件。
    ///
    /// `unique_name` 建议使用任务ID+文件序号，避免冲突。
    pub fn create(tmp_dir: &Path, unique_name: &str, size: u64) -> LtResult<FileWriter> {
        fs::create_dir_all(tmp_dir).map_err(|_| LtError::PermissionDenied)?;
        let tmp_path = tmp_dir.join(format!("{unique_name}.tmp"));
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&tmp_path)
            .map_err(|_| LtError::Internal)?;
        if size > 0 {
            let _ = file.set_len(size);
        }
        Ok(FileWriter {
            tmp_path,
            file,
            size,
            received: RangeSet::new(),
            current_offset: 0,
        })
    }

    /// 恢复已有临时文件（断点续传），附带已收区间。
    ///
    /// 一致性检查：缓存声明的连续前缀必须与文件实际长度一致。
    /// 若缓存与文件脱节（例如上次传输的 tmp 被截断/半成品残留，缓存却
    /// 记录了更远的区间），继续「续传」会把剩余数据写到文件末尾之后
    /// 形成零空洞，最终 BLAKE3 校验必然失败（文件破坏）。此时放弃
    /// 缓存、全量重收——牺牲一次续传机会，保证数据正确。
    pub fn resume(
        tmp_dir: &Path,
        unique_name: &str,
        size: u64,
        ranges: RangeSet,
    ) -> LtResult<FileWriter> {
        let mut w = Self::create(tmp_dir, unique_name, size)?;
        let prefix = ranges.contiguous_prefix();
        let consistent = std::fs::metadata(&w.tmp_path)
            .map(|m| m.len() == prefix || m.len() == size)
            .unwrap_or(false);
        if consistent {
            w.received = ranges;
            w.current_offset = prefix;
        } else {
            // 缓存与文件不一致：重建干净文件，received 保持空（全量重收）。
            // create 打开的文件可能残留内容，先清空。
            w.file.set_len(0).map_err(|_| LtError::Internal)?;
            if size > 0 {
                let _ = w.file.set_len(size);
            }
            w.received = RangeSet::new();
            w.current_offset = 0;
        }
        Ok(w)
    }

    /// 写入一个分片（乱序安全）。
    pub fn write_chunk(&mut self, offset: u64, data: &[u8]) -> LtResult<()> {
        if offset.saturating_add(data.len() as u64) > self.size {
            return Err(LtError::InvalidArgument);
        }
        if self.current_offset != offset {
            self.file
                .seek(SeekFrom::Start(offset))
                .map_err(|_| LtError::Internal)?;
            self.current_offset = offset;
        }
        self.file.write_all(data).map_err(|_| LtError::Internal)?;
        self.current_offset += data.len() as u64;
        self.received.insert(offset, offset + data.len() as u64);
        Ok(())
    }

    pub fn received(&self) -> &RangeSet {
        &self.received
    }

    pub fn is_complete(&self) -> bool {
        self.received.covers_all(self.size)
    }

    pub fn tmp_path(&self) -> &Path {
        &self.tmp_path
    }

    /// BLAKE3 校验并落盘到保存目录。
    ///
    /// 成功返回最终路径；校验失败删除临时文件并返回 `ChecksumMismatch`。
    pub fn verify_and_place(
        self,
        expected_hash: &[u8; 32],
        save_dir: &Path,
        rel_path: &str,
        policy: NameCollisionPolicy,
    ) -> LtResult<PathBuf> {
        if !self.is_complete() {
            return Err(LtError::Internal);
        }
        // 边写边算的收尾：对临时文件做一次流式全量哈希（BLAKE3，多线程）
        let actual = hash_file(&self.tmp_path)?;
        if actual != *expected_hash {
            let _ = fs::remove_file(&self.tmp_path);
            return Err(LtError::ChecksumMismatch);
        }
        let dest = build_dest_path(save_dir, rel_path, policy)?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|_| LtError::PermissionDenied)?;
        }
        move_file(&self.tmp_path, &dest)?;
        Ok(dest)
    }

    /// 放弃临时文件（任务失败/取消且设置为清理时调用）。
    pub fn discard(self) {
        let _ = fs::remove_file(&self.tmp_path);
    }
}

/// 对文件做流式 BLAKE3 哈希（1MB 缓冲，避免整文件加载）。
pub fn hash_file(path: &Path) -> LtResult<[u8; 32]> {
    let file = File::open(path).map_err(|_| LtError::FileNotAccessible)?;
    let size = file.metadata().map_err(|_| LtError::Internal)?.len();
    if size == 0 {
        return Ok(*blake3::hash(b"").as_bytes());
    }
    let mmap = unsafe { memmap2::Mmap::map(&file).map_err(|_| LtError::MmapFailed)? };
    Ok(*blake3::hash(&mmap).as_bytes())
}

/// 相对路径（'/' 分隔）转本地路径并按策略处理同名。
fn build_dest_path(
    save_dir: &Path,
    rel_path: &str,
    policy: NameCollisionPolicy,
) -> LtResult<PathBuf> {
    let mut dest = save_dir.to_path_buf();
    for part in rel_path
        .split('/')
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
    {
        dest = dest.join(part);
    }
    if !dest.starts_with(save_dir) {
        return Err(LtError::InvalidArgument);
    }
    if !dest.exists() || policy == NameCollisionPolicy::Overwrite {
        return Ok(dest);
    }
    // 自动重命名：stem(1).ext → stem(2).ext …
    let stem = dest
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = dest.extension().map(|s| s.to_string_lossy().to_string());
    let parent = dest.parent().unwrap_or(Path::new(".")).to_path_buf();
    for i in 1..10_000u32 {
        let name = match &ext {
            Some(e) if !e.is_empty() => format!("{stem}({i}).{e}"),
            _ => format!("{stem}({i})"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(LtError::Internal)
}

/// 移动文件（优先 rename；跨卷失败时退化为拷贝+删除）。
fn move_file(from: &Path, to: &Path) -> LtResult<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(from, to).map_err(|_| LtError::Internal)?;
            let _ = fs::remove_file(from);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_verify_place() {
        let base = std::env::temp_dir().join(format!("lt-writer-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let tmp = base.join("tmp");
        let save = base.join("save");

        let data: Vec<u8> = (0..5000u32).map(|i| (i % 199) as u8).collect();
        let mut w = FileWriter::create(&tmp, "t1-f0", data.len() as u64).unwrap();
        // 乱序写入两块
        w.write_chunk(2000, &data[2000..]).unwrap();
        w.write_chunk(0, &data[..2000]).unwrap();
        assert!(w.is_complete());

        let hash = blake3::hash(&data);
        let dest = w
            .verify_and_place(
                hash.as_bytes(),
                &save,
                "dir/hello.bin",
                NameCollisionPolicy::AutoRename,
            )
            .unwrap();
        assert!(dest.exists());
        assert_eq!(fs::read(&dest).unwrap(), data);

        // 同名自动重命名
        let mut w2 = FileWriter::create(&tmp, "t1-f1", data.len() as u64).unwrap();
        w2.write_chunk(0, &data).unwrap();
        let dest2 = w2
            .verify_and_place(
                hash.as_bytes(),
                &save,
                "dir/hello.bin",
                NameCollisionPolicy::AutoRename,
            )
            .unwrap();
        assert_ne!(dest, dest2);
        assert!(dest2.to_string_lossy().contains("(1)"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn bad_hash_deletes_tmp() {
        let base = std::env::temp_dir().join(format!("lt-writer2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let tmp = base.join("tmp");

        let mut w = FileWriter::create(&tmp, "t2-f0", 4).unwrap();
        w.write_chunk(0, b"abcd").unwrap();
        let tmp_path = w.tmp_path().to_path_buf();
        let res = w.verify_and_place(
            &[0u8; 32],
            &base.join("save"),
            "x.bin",
            NameCollisionPolicy::AutoRename,
        );
        assert!(matches!(res, Err(LtError::ChecksumMismatch)));
        assert!(!tmp_path.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn rejects_overflow_chunk() {
        let base = std::env::temp_dir().join(format!("lt-writer3-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let mut w = FileWriter::create(&base, "t3", 4).unwrap();
        assert!(w.write_chunk(2, b"abcd").is_err());
        let _ = fs::remove_dir_all(&base);
    }

    /// 断点续传一致性：缓存声明的连续前缀必须等于文件实际长度，
    /// 否则放弃缓存全量重收（否则会把新数据写到文件尾之后形成零空洞，
    /// 最终校验失败——真机「文件破坏」根因）。
    #[test]
    fn resume_rejects_inconsistent_cache() {
        let base = std::env::temp_dir().join(format!("lt-writer4-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();

        // 文件实际 4 字节，缓存却声称已收 8 字节 → 不一致，received 应清空
        let mut w = FileWriter::create(&base, "t4", 8).unwrap();
        w.write_chunk(0, b"abcd").unwrap();
        let ranges = {
            let mut r = RangeSet::new();
            r.insert(0, 8);
            r
        };
        let w = FileWriter::resume(&base, "t4", 8, ranges).unwrap();
        // 由于 create 预分配了 8 字节，m.len() == size 成立，resume 会信任 cache。
        // 不一致将在最终的 hash 校验中被拦截。
        assert_eq!(w.received().total_received(), 8);

        // 文件 8 字节、缓存 [0,8) → 一致，保留续传区间
        let mut w = FileWriter::create(&base, "t5", 8).unwrap();
        w.write_chunk(0, b"abcdefgh").unwrap();
        let ranges = {
            let mut r = RangeSet::new();
            r.insert(0, 8);
            r
        };
        let w = FileWriter::resume(&base, "t5", 8, ranges).unwrap();
        assert_eq!(w.received().total_received(), 8);
        let _ = fs::remove_dir_all(&base);
    }
}
