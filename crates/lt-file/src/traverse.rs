//! 文件夹递归遍历（实施方案 8.1）。
//!
//! - 自动展开子目录、统计总大小、生成带相对路径的任务列表；
//! - 符号链接/管道/设备文件默认跳过并计入"跳过清单"；
//! - Windows 长路径由 Rust std 原生 \\?\ 语义处理，遍历层不截断。

use std::path::{Path, PathBuf};

use lt_utils::{LtError, LtResult};

/// 单个待传输文件项。
#[derive(Debug, Clone)]
pub struct TransferItem {
    /// 绝对路径
    pub abs_path: PathBuf,
    /// 相对路径（'/' 分隔）
    pub rel_path: String,
    /// 字节数
    pub size: u64,
    /// 修改时间（unix 秒）
    pub mtime_unix: i64,
    /// 头部数据哈希
    pub head_hash: [u8; 16],
}

/// 被跳过的条目（任务完成时汇总通知 UI）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkippedEntry {
    pub path: String,
    pub reason: String,
}

/// 遍历结果。
#[derive(Debug, Default)]
pub struct TraverseResult {
    pub items: Vec<TransferItem>,
    pub total_size: u64,
    pub skipped: Vec<SkippedEntry>,
}

/// 遍历一组文件/目录路径，生成传输项列表。
///
/// 单个文件：相对路径即文件名；目录：相对路径以目录名为根。
pub fn traverse(paths: &[PathBuf]) -> LtResult<TraverseResult> {
    if paths.is_empty() {
        return Err(LtError::InvalidArgument);
    }
    let mut result = TraverseResult::default();
    for path in paths {
        if !path.exists() {
            return Err(LtError::FileNotAccessible);
        }
        let meta = std::fs::symlink_metadata(path).map_err(|_| LtError::FileNotAccessible)?;
        if meta.is_symlink() {
            result.skipped.push(SkippedEntry {
                path: display(path),
                reason: "符号链接".into(),
            });
            continue;
        }
        if meta.is_file() {
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| display(path));
            push_item(&mut result, path.to_path_buf(), &name)?;
        } else if meta.is_dir() {
            let root_name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| display(path));
            walk_dir(&mut result, path, &root_name)?;
        } else {
            result.skipped.push(SkippedEntry {
                path: display(path),
                reason: "非常规文件（管道/设备等）".into(),
            });
        }
    }
    if result.items.is_empty() {
        return Err(LtError::FileNotAccessible);
    }
    Ok(result)
}

fn walk_dir(result: &mut TraverseResult, dir: &Path, rel: &str) -> LtResult<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => {
            result.skipped.push(SkippedEntry {
                path: display(dir),
                reason: "无读取权限".into(),
            });
            return Ok(());
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => {
                result.skipped.push(SkippedEntry {
                    path: display(&path),
                    reason: "无法读取元数据".into(),
                });
                continue;
            }
        };
        let child_rel = format!(
            "{}/{}",
            rel,
            path.file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        );
        if meta.is_symlink() {
            result.skipped.push(SkippedEntry {
                path: child_rel,
                reason: "符号链接".into(),
            });
        } else if meta.is_file() {
            push_item(result, path, &child_rel)?;
        } else if meta.is_dir() {
            walk_dir(result, &path, &child_rel)?;
        } else {
            result.skipped.push(SkippedEntry {
                path: child_rel,
                reason: "非常规文件（管道/设备等）".into(),
            });
        }
    }
    Ok(())
}

fn push_item(result: &mut TraverseResult, abs_path: PathBuf, rel_path: &str) -> LtResult<()> {
    let ident = crate::identity::FileIdentity::from_fs(&abs_path, rel_path)?;
    result.total_size += ident.size;
    result.items.push(TransferItem {
        abs_path,
        rel_path: ident.rel_path,
        size: ident.size,
        mtime_unix: ident.mtime_unix,
        head_hash: ident.head_hash,
    });
    Ok(())
}

fn display(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lt-traverse-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub/deep")).unwrap();
        fs::write(dir.join("root.txt"), b"hello").unwrap();
        fs::write(dir.join("sub/a.bin"), vec![0u8; 1024]).unwrap();
        fs::write(dir.join("sub/deep/b.bin"), vec![1u8; 2048]).unwrap();
        dir
    }

    #[test]
    fn traverse_tree() {
        let dir = fixture("tree");
        let res = traverse(std::slice::from_ref(&dir)).unwrap();
        assert_eq!(res.items.len(), 3);
        assert_eq!(res.total_size, 5 + 1024 + 2048);
        let rels: Vec<&str> = res.items.iter().map(|i| i.rel_path.as_str()).collect();
        assert!(rels.iter().any(|r| r.ends_with("root.txt")));
        assert!(rels.iter().any(|r| r.contains("/sub/deep/b.bin")));
        assert!(res.skipped.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn single_file() {
        let dir = fixture("single");
        let f = dir.join("root.txt");
        let res = traverse(&[f]).unwrap();
        assert_eq!(res.items.len(), 1);
        assert_eq!(res.items[0].rel_path, "root.txt");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_path_errors() {
        let res = traverse(&[PathBuf::from("/nonexistent/xyz")]);
        assert!(matches!(res, Err(LtError::FileNotAccessible)));
    }
}
