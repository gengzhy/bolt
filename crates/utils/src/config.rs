//! 应用配置（FFI `bt_set_config` / `bt_get_config` 的后端）。
//!
//! key 集合：device_name、save_dir、stealth_mode、auto_accept_trusted、concurrency
//! 以及端口等传输参数。配置持久化为 JSON。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{BtError, BtResult};
use crate::DEFAULT_PORT;

/// 默认数据目录：<data_dir>/bolt
pub fn default_data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("bolt")
}

/// 全局应用配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 设备显示名称
    #[serde(default = "default_device_name")]
    pub device_name: String,
    /// 接收文件保存目录
    #[serde(default = "default_save_dir")]
    pub save_dir: PathBuf,
    /// 隐身模式：暂停 mDNS 广播与 UDP 探测应答
    #[serde(default)]
    pub stealth_mode: bool,
    /// 已配对设备自动接收
    #[serde(default)]
    pub auto_accept_trusted: bool,
    /// 单任务文件并发数（默认 4）
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    /// 监听端口（默认 8899，占用时自动递增尝试端口池）
    #[serde(default = "default_port")]
    pub listen_port: u16,
    /// 分片大小（字节，默认 1MB；RTT 异常时底层降至 256KB）
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    /// 应用私有数据目录（证书、信任库、断点缓存、日志）
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    /// 传输协议（二选一，默认 QUIC）：true=QUIC / false=TCP；
    /// 选定协议不可用时直接报错，不自动切换
    #[serde(default = "default_true")]
    pub prefer_quic: bool,
    /// mDNS 自动发现（默认开启）
    #[serde(default = "default_true")]
    pub use_mdns: bool,
    /// 接收文件同名冲突策略：`rename`（自动重命名）/ `overwrite`（覆盖）
    #[serde(default = "default_collision")]
    pub collision: String,
    /// Windows 桌面端关闭窗口时最小化到系统托盘（默认 false）
    #[serde(default)]
    pub minimize_to_tray: bool,
}

fn default_true() -> bool {
    true
}
fn default_collision() -> String {
    "rename".into()
}
fn default_concurrency() -> usize {
    4
}
fn default_port() -> u16 {
    DEFAULT_PORT
}
fn default_chunk_size() -> usize {
    crate::constants::DEFAULT_CHUNK_SIZE
}
fn default_save_dir() -> PathBuf {
    // Android：公共下载目录 /Download/Bolt（需「所有文件访问」权限）；
    // iOS：私有沙盒目录下的 received（由 Swift 层初始化传参 Documents 目录）；
    // 其它平台：系统下载目录（Windows 即 C:\Users\<用户名>\Downloads，用户名动态获取），
    // 取不到时回落数据目录下的 received。
    #[cfg(target_os = "android")]
    {
        return PathBuf::from("/storage/emulated/0/Download/Bolt");
    }
    #[cfg(target_os = "ios")]
    {
        return default_data_dir().join("received");
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    dirs::download_dir().unwrap_or_else(|| default_data_dir().join("received"))
}

impl Default for AppConfig {
    fn default() -> Self {
        let data_dir = default_data_dir();
        let save_dir = default_save_dir();
        Self {
            device_name: default_device_name(),
            save_dir,
            stealth_mode: false,
            auto_accept_trusted: false,
            concurrency: default_concurrency(),
            listen_port: DEFAULT_PORT,
            chunk_size: default_chunk_size(),
            data_dir,
            prefer_quic: true,
            use_mdns: true,
            collision: default_collision(),
            minimize_to_tray: false,
        }
    }
}

fn default_device_name() -> String {
    // Windows 取系统设备名称；Android 无 COMPUTERNAME/HOSTNAME 环境变量时
    // 回落 "device"，由 App 层在首启时改写为「关于手机」中的设备名称。
    bt_hostname()
}

fn bt_hostname() -> String {
    // Windows 取「设置→系统→关于」中的设备名称（含 DNS 后缀的物理机全限定名）；
    // Linux 取 /etc/hostname；其他平台取 COMPUTERNAME / HOSTNAME 环境变量，
    // Android 上回落 "device"，由 App 层改写为「关于手机」中的设备名称。
    #[cfg(target_os = "windows")]
    if let Some(name) = windows_device_name() {
        if !name.is_empty() {
            return name;
        }
    }
    #[cfg(target_os = "linux")]
    if let Ok(name) = std::fs::read_to_string("/etc/hostname") {
        let name = name.trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "device".into())
}

/// Windows 设备名称 = `GetComputerNameExW(ComputerNamePhysicalDnsFullyQualified)`，
/// 与「设置→系统→关于→设备名称」显示一致（独立机无后缀，域机带 DNS 后缀）。
#[cfg(target_os = "windows")]
fn windows_device_name() -> Option<String> {
    #[link(name = "kernel32")]
    extern "system" {
        #[allow(non_snake_case)]
        fn GetComputerNameExW(name_type: u32, buffer: *mut u16, n_size: *mut u32) -> i32;
    }
    const COMPUTER_NAME_PHYSICAL_DNS_FULLY_QUALIFIED: u32 = 7;
    unsafe {
        let mut size = 0u32;
        // 第一次调用取所需缓冲长度（失败是预期行为）
        GetComputerNameExW(
            COMPUTER_NAME_PHYSICAL_DNS_FULLY_QUALIFIED,
            std::ptr::null_mut(),
            &mut size,
        );
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u16; size as usize];
        if GetComputerNameExW(
            COMPUTER_NAME_PHYSICAL_DNS_FULLY_QUALIFIED,
            buf.as_mut_ptr(),
            &mut size,
        ) == 0
        {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..size as usize]))
    }
}

impl AppConfig {
    /// 从 JSON 合并配置（仅覆盖提供的字段），返回新配置。
    pub fn merge_json(base: AppConfig, json: &str) -> BtResult<AppConfig> {
        let patch: serde_json::Value =
            serde_json::from_str(json).map_err(|_| BtError::InvalidArgument)?;
        let mut value = serde_json::to_value(&base).map_err(|_| BtError::Internal)?;
        merge_value(&mut value, &patch);
        serde_json::from_value(value).map_err(|_| BtError::InvalidArgument)
    }

    /// 读取单个 key（返回 JSON 值字符串）。
    pub fn get_key(&self, key: &str) -> BtResult<String> {
        let v = serde_json::to_value(self).map_err(|_| BtError::Internal)?;
        let field = match key {
            "device_name"
            | "save_dir"
            | "stealth_mode"
            | "auto_accept_trusted"
            | "concurrency"
            | "listen_port"
            | "chunk_size"
            | "data_dir"
            | "prefer_quic"
            | "use_mdns"
            | "collision"
            | "minimize_to_tray" => key,
            _ => return Err(BtError::InvalidArgument),
        };
        let item = v.get(field).ok_or(BtError::InvalidArgument)?;
        serde_json::to_string(item).map_err(|_| BtError::Internal)
    }

    /// 写入单个 key。
    pub fn set_key(&mut self, key: &str, value_json: &str) -> BtResult<()> {
        let parsed: serde_json::Value =
            serde_json::from_str(value_json).map_err(|_| BtError::InvalidArgument)?;
        let mut v = serde_json::to_value(&*self).map_err(|_| BtError::Internal)?;
        let obj = v.as_object_mut().ok_or(BtError::Internal)?;
        if !obj.contains_key(key) {
            return Err(BtError::InvalidArgument);
        }
        obj.insert(key.to_string(), parsed);
        *self = serde_json::from_value(v).map_err(|_| BtError::InvalidArgument)?;
        Ok(())
    }

    /// 持久化到 data_dir/config.json。
    pub fn save(&self) -> BtResult<()> {
        let path = self.data_dir.join("config.json");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| BtError::Internal)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|_| BtError::Internal)?;
        std::fs::write(&path, json).map_err(|_| BtError::Internal)?;
        Ok(())
    }

    /// 从 data_dir/config.json 加载，不存在则返回默认。
    /// 加载后强制 data_dir 跟随调用参数；save_dir 若仍指向默认数据目录则一并迁移。
    pub fn load(data_dir: &Path) -> AppConfig {
        let path = data_dir.join("config.json");
        let mut cfg = match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                tracing::warn!(?path, error = %e, "config corrupt, using defaults");
                default_for_dir(data_dir)
            }),
            Err(_) => default_for_dir(data_dir),
        };
        let default_base = default_data_dir();
        if let Ok(rel) = cfg.save_dir.strip_prefix(&default_base) {
            cfg.save_dir = data_dir.join(rel);
        }
        // 旧版默认接收目录是 data_dir/received；新版改为系统下载目录，
        // 仍指向旧默认的旧安装一次性迁移（用户自定义目录不受影响）。
        if cfg.save_dir == data_dir.join("received") {
            cfg.save_dir = default_save_dir();
        }
        cfg.data_dir = data_dir.to_path_buf();
        // 旧版默认名是「{hostname}-{4位hex}」；按需求改为取系统设备名称。
        // 历史 hostname 可能来自 COMPUTERNAME 或新版设备名称，逐一匹配候选值
        // （忽略大小写）；用户自定义名称不受迁移影响。
        let host = bt_hostname();
        let mut candidates = vec![host.clone()];
        for key in ["COMPUTERNAME", "HOSTNAME"] {
            if let Ok(v) = std::env::var(key) {
                candidates.push(v);
            }
        }
        candidates.push("device".into());
        let lower = cfg.device_name.to_ascii_lowercase();
        if candidates.iter().any(|cand| {
            let prefix = format!("{}-", cand.to_ascii_lowercase());
            lower
                .strip_prefix(&prefix)
                .is_some_and(|s| s.len() == 4 && s.chars().all(|ch| ch.is_ascii_hexdigit()))
        }) {
            cfg.device_name = host;
        }
        cfg
    }
}

/// 默认配置（数据目录跟随 data_dir；接收目录取平台默认下载目录）。
fn default_for_dir(data_dir: &Path) -> AppConfig {
    AppConfig {
        data_dir: data_dir.to_path_buf(),
        save_dir: default_save_dir(),
        ..Default::default()
    }
}

fn merge_value(base: &mut serde_json::Value, patch: &serde_json::Value) {
    match (base, patch) {
        (serde_json::Value::Object(b), serde_json::Value::Object(p)) => {
            for (k, v) in p {
                merge_value(b.entry(k.clone()).or_insert(serde_json::Value::Null), v);
            }
        }
        (b, p) => *b = p.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_and_keys() {
        let base = AppConfig::default();
        let merged =
            AppConfig::merge_json(base, r#"{"device_name":"我的电脑","stealth_mode":true}"#)
                .unwrap();
        assert_eq!(merged.device_name, "我的电脑");
        assert!(merged.stealth_mode);
        assert_eq!(merged.get_key("device_name").unwrap(), "\"我的电脑\"");

        let mut c = merged;
        c.set_key("concurrency", "8").unwrap();
        assert_eq!(c.concurrency, 8);
        assert!(c.set_key("not_a_key", "1").is_err());
        assert!(c.set_key("concurrency", "not-json").is_err());
    }

    #[test]
    fn migrates_legacy_default_name() {
        let dir = std::env::temp_dir().join(format!("bt_cfg_test_{}", crate::id::new_uuid()));
        std::fs::create_dir_all(&dir).unwrap();
        let host = bt_hostname();

        // 旧版默认名 {host}-{4位hex} → 迁移为 {host}
        let mut cfg = default_for_dir(&dir);
        cfg.device_name = format!("{host}-ab12");
        cfg.save().unwrap();
        assert_eq!(AppConfig::load(&dir).device_name, host);

        // 用户自定义名称不受迁移影响
        let mut cfg2 = default_for_dir(&dir);
        cfg2.device_name = "我的电脑".into();
        cfg2.save().unwrap();
        assert_eq!(AppConfig::load(&dir).device_name, "我的电脑");

        // 旧版回落前缀 "device-" 同样迁移（忽略大小写）
        let mut cfg3 = default_for_dir(&dir);
        cfg3.device_name = "DeViCe-ab12".into();
        cfg3.save().unwrap();
        assert_eq!(AppConfig::load(&dir).device_name, host);

        std::fs::remove_dir_all(&dir).ok();
    }
}
