//! 写入与合并（原子落盘与同名处理）。
//!
//! 分片写入私有临时目录 `.tmp` 文件，全部接收且 BLAKE3 校验通过后
//! 一次性原子移动至用户保存目录；同名默认自动重命名 `(1)、(2)…`。
//! 传输取消或校验失败时立即删除临时文件，杜绝磁盘脏数据残留。

use std::fs::{self, File};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use lt_utils::{LtError, LtResult};

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
    written_bytes: u64,
    current_offset: u64,
    hasher: Option<blake3::Hasher>,
}

impl FileWriter {
    /// 在临时目录创建接收临时文件。
    pub fn create(tmp_dir: &Path, unique_name: &str, size: u64) -> LtResult<FileWriter> {
        fs::create_dir_all(tmp_dir).map_err(|_| LtError::PermissionDenied)?;
        let tmp_path = tmp_dir.join(format!("{unique_name}.tmp"));
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)
            .map_err(|_| LtError::Internal)?;
        if size > 0 {
            let _ = file.set_len(size);
        }
        Ok(FileWriter {
            tmp_path,
            file,
            size,
            written_bytes: 0,
            current_offset: 0,
            hasher: Some(blake3::Hasher::new()),
        })
    }

    /// 写入一个分片。
    pub fn write_chunk(&mut self, offset: u64, data: &[u8]) -> LtResult<()> {
        let len = data.len() as u64;
        if offset.saturating_add(len) > self.size {
            return Err(LtError::InvalidArgument);
        }
        if self.current_offset != offset {
            self.file
                .seek(SeekFrom::Start(offset))
                .map_err(|_| LtError::Internal)?;
            self.current_offset = offset;
            // 发生非连续跳跃，流水线流式哈希失效，落盘校验时自动回退为 mmap 全量比对
            self.hasher = None;
        }
        self.file.write_all(data).map_err(|_| LtError::Internal)?;
        self.current_offset += len;
        self.written_bytes = self.written_bytes.max(offset + len);
        if let Some(h) = &mut self.hasher {
            h.update(data);
        }
        Ok(())
    }

    pub fn written_bytes(&self) -> u64 {
        self.written_bytes
    }

    pub fn is_complete(&self) -> bool {
        self.written_bytes == self.size
    }

    pub fn tmp_path(&self) -> &Path {
        &self.tmp_path
    }

    /// BLAKE3 校验并原子落盘到保存目录。
    /// 若写入过程完全连续，直接从增量流水线哈希器中 0ms 瞬间获取哈希比对，
    /// 彻底消除大文件在传输结尾重新扫描几 GB 磁盘文件带来的数秒卡顿。
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
        let FileWriter { tmp_path, file, hasher, .. } = self;
        let _ = file.sync_all();
        drop(file);

        let actual = if let Some(h) = hasher {
            *h.finalize().as_bytes()
        } else {
            hash_file(&tmp_path)?
        };

        if actual != *expected_hash {
            let _ = fs::remove_file(&tmp_path);
            return Err(LtError::ChecksumMismatch);
        }
        let dest = build_dest_path(save_dir, rel_path, policy)?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|_| LtError::PermissionDenied)?;
        }
        move_file(&tmp_path, &dest)?;
        Ok(dest)
    }

    /// 放弃并物理删除临时文件（任务失败/取消时调用，零磁盘垃圾残留）。
    pub fn discard(self) {
        let FileWriter { tmp_path, file, .. } = self;
        drop(file);
        let _ = fs::remove_file(&tmp_path);
    }
}

/// 对文件做流式 BLAKE3 哈希。
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
        w.write_chunk(0, &data[..2000]).unwrap();
        w.write_chunk(2000, &data[2000..]).unwrap();
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
}
