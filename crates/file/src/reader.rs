//! 分片读取（实施方案 8.1）：memmap2 内存映射 + BLAKE3 流式哈希。
//!
//! 发送端"边读边算"：按序读取分片同时喂入哈希器，
//! 不做传输前全量预哈希，100GB 文件即时可发。

use std::fs::File;
use std::path::Path;

use memmap2::Mmap;

use utils::{LtError, LtResult};

/// mmap 分片读取器（只读，顺序推进）。
pub struct FileReader {
    _file: File,
    mmap: Option<Mmap>,
    size: u64,
    pos: u64,
    hasher: blake3::Hasher,
}

impl FileReader {
    /// 打开文件并建立内存映射。空文件不建映射（哈希为空的 BLAKE3）。
    pub fn open(path: &Path) -> LtResult<FileReader> {
        let file = File::open(path).map_err(|_| LtError::FileNotAccessible)?;
        let size = file
            .metadata()
            .map_err(|_| LtError::FileNotAccessible)?
            .len();
        let mmap = if size > 0 {
            Some(unsafe { Mmap::map(&file).map_err(|_| LtError::MmapFailed)? })
        } else {
            None
        };
        Ok(FileReader {
            _file: file,
            mmap,
            size,
            pos: 0,
            hasher: blake3::Hasher::new(),
        })
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn remaining(&self) -> u64 {
        self.size.saturating_sub(self.pos)
    }

    /// 读取下一分片（≤chunk_size），同时更新全文件哈希。
    /// 返回 None 表示已到文件尾。
    pub fn next_chunk(&mut self, chunk_size: usize) -> LtResult<Option<&[u8]>> {
        if self.pos >= self.size {
            return Ok(None);
        }
        let take = (chunk_size as u64).min(self.size - self.pos) as usize;
        let start = self.pos as usize;
        let data = match &self.mmap {
            Some(m) => &m[start..start + take],
            None => return Ok(None),
        };
        self.hasher.update(data);
        self.pos += take as u64;
        Ok(Some(data))
    }

    /// 消费读取器，返回全文件 BLAKE3（32 字节）。
    pub fn finalize(self) -> [u8; 32] {
        *self.hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn chunked_read_matches_full_hash() {
        let dir = std::env::temp_dir().join(format!("lt-reader-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.bin");
        let data: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        fs::write(&path, &data).unwrap();

        let mut reader = FileReader::open(&path).unwrap();
        let mut chunks = 0;
        while reader.next_chunk(1024).unwrap().is_some() {
            chunks += 1;
        }
        assert_eq!(chunks, (10_000 + 1023) / 1024);
        let streamed = reader.finalize();
        assert_eq!(streamed, *blake3::hash(&data).as_bytes());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_file() {
        let dir = std::env::temp_dir().join(format!("lt-reader2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.bin");
        fs::write(&path, b"").unwrap();
        let mut reader = FileReader::open(&path).unwrap();
        assert!(reader.next_chunk(1024).unwrap().is_none());
        let h = reader.finalize();
        assert_eq!(h, *blake3::hash(b"").as_bytes());
        let _ = fs::remove_dir_all(&dir);
    }
}
