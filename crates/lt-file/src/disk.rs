//! 磁盘空间预检（实施方案 8.3）。
//!
//! 任务启动前：剩余空间 ≥ 总大小 × 1.05 + 512MB，否则拒绝启动（-5）；
//! 接收端每完成一个文件复检一次，不足时暂停并告警而非直接失败。

use std::path::Path;

use lt_utils::{LtError, LtResult};

/// 预检余量系数与固定缓冲。
const SAFETY_FACTOR_NUM: u64 = 105; // ×1.05
const SAFETY_FACTOR_DEN: u64 = 100;
const FIXED_RESERVE: u64 = 512 * 1024 * 1024; // 512MB

/// 查询路径所在卷的可用空间（字节）。
pub fn available_space(path: &Path) -> LtResult<u64> {
    #[cfg(windows)]
    {
        windows_free_space(path).ok_or(LtError::Internal)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        // V1 非 Windows 平台（Android）由 JNI 侧注入实际值；此处不阻断
        tracing::debug!("disk space check not available on this platform, skipping");
        Ok(u64::MAX)
    }
}

#[cfg(windows)]
fn windows_free_space(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            dir: *const u16,
            free_bytes_available_to_caller: *mut u64,
            total_number_of_bytes: *mut u64,
            total_number_of_free_bytes: *mut u64,
        ) -> i32;
    }

    // 目录必须存在才能查询；退化为查询最近的已存在祖先目录
    let mut probe = path.to_path_buf();
    loop {
        if probe.exists() {
            break;
        }
        probe = probe.parent()?.to_path_buf();
    }
    let wide: Vec<u16> = probe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut avail = 0u64;
    let mut total = 0u64;
    let mut free = 0u64;
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, &mut total, &mut free) };
    if ok != 0 {
        Some(avail)
    } else {
        None
    }
}

/// 任务启动前预检：不足返回 `LtError::DiskFull`。
pub fn precheck(dir: &Path, total_size: u64) -> LtResult<()> {
    let free = available_space(dir)?;
    let need = total_size / SAFETY_FACTOR_DEN * SAFETY_FACTOR_NUM + FIXED_RESERVE;
    if free < need {
        tracing::warn!(free, need, "disk precheck failed");
        return Err(LtError::DiskFull);
    }
    Ok(())
}

/// 单文件复检（接收端每完成一个文件调用）：不足返回 false，由上层暂停任务告警。
pub fn recheck(dir: &Path, pending_size: u64) -> bool {
    match available_space(dir) {
        Ok(free) => free >= pending_size / SAFETY_FACTOR_DEN * SAFETY_FACTOR_NUM + FIXED_RESERVE,
        Err(_) => true, // 查询失败不阻断（由上层告警）
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precheck_huge_fails() {
        let tmp = std::env::temp_dir();
        // 10EB 必然不足
        assert!(matches!(
            precheck(&tmp, u64::MAX / 2),
            Err(LtError::DiskFull)
        ));
    }

    #[test]
    fn precheck_zero_ok() {
        let tmp = std::env::temp_dir();
        assert!(precheck(&tmp, 0).is_ok());
    }
}
