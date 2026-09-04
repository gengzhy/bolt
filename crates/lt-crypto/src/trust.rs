//! TOFU 信任库（实施方案 5.2）。
//!
//! 首次配对成功后互存对方证书指纹；后续连接自动校验：
//! 不一致立即断开并报 -14（疑似中间人）。配对记录可在设置页清除。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use lt_utils::{LtError, LtResult};

/// 一条配对信任记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedDevice {
    pub uuid: String,
    pub fingerprint: String,
    pub name: String,
    /// 配对时间（unix 秒）
    pub paired_at: u64,
    /// 是否允许该设备自动接收（可随时关闭）
    #[serde(default)]
    pub auto_receive: bool,
}

/// 指纹校验结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustStatus {
    /// 已配对且指纹一致
    Trusted,
    /// 陌生设备，需走配对流程
    Unknown,
    /// 指纹变更：疑似中间人（错误码 -14）
    Changed,
}

/// 持久化信任库（`data_dir/trusted_devices.json`）。
pub struct TrustStore {
    path: PathBuf,
    devices: Mutex<HashMap<String, TrustedDevice>>,
}

impl TrustStore {
    /// 从磁盘加载（不存在则为空）。
    pub fn load(data_dir: &Path) -> LtResult<TrustStore> {
        let path = data_dir.join("trusted_devices.json");
        let mut devices = HashMap::new();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(s) => {
                    if let Ok(list) = serde_json::from_str::<Vec<TrustedDevice>>(&s) {
                        for d in list {
                            devices.insert(d.uuid.clone(), d);
                        }
                    } else {
                        tracing::warn!("trusted_devices.json corrupt, starting empty");
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "read trust store failed");
                }
            }
        }
        Ok(TrustStore {
            path,
            devices: Mutex::new(devices),
        })
    }

    fn persist(&self, devices: &HashMap<String, TrustedDevice>) -> LtResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| LtError::Internal)?;
        }
        let list: Vec<&TrustedDevice> = devices.values().collect();
        let s = serde_json::to_string_pretty(&list).map_err(|_| LtError::Internal)?;
        fs::write(&self.path, s).map_err(|_| LtError::Internal)
    }

    /// 校验设备指纹：已配对比对 / 未知 / 变更。
    pub fn check(&self, uuid: &str, fingerprint: &str) -> TrustStatus {
        let fp = crate::fingerprint::normalize(fingerprint);
        let devices = self.devices.lock().unwrap();
        match devices.get(uuid) {
            None => TrustStatus::Unknown,
            Some(d) if crate::fingerprint::normalize(&d.fingerprint) == fp => TrustStatus::Trusted,
            Some(_) => TrustStatus::Changed,
        }
    }

    /// 配对成功后写入/更新记录。
    pub fn add(&self, uuid: &str, fingerprint: &str, name: &str) -> LtResult<()> {
        let mut devices = self.devices.lock().unwrap();
        devices.insert(
            uuid.to_string(),
            TrustedDevice {
                uuid: uuid.to_string(),
                fingerprint: crate::fingerprint::normalize(fingerprint),
                name: name.to_string(),
                paired_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                auto_receive: false,
            },
        );
        self.persist(&devices)
    }

    /// 设置/关闭某设备的"自动接收"。
    pub fn set_auto_receive(&self, uuid: &str, enabled: bool) -> LtResult<()> {
        let mut devices = self.devices.lock().unwrap();
        if let Some(d) = devices.get_mut(uuid) {
            d.auto_receive = enabled;
            return self.persist(&devices);
        }
        Err(LtError::InvalidArgument)
    }

    pub fn is_auto_receive(&self, uuid: &str) -> bool {
        self.devices
            .lock()
            .unwrap()
            .get(uuid)
            .map(|d| d.auto_receive)
            .unwrap_or(false)
    }

    /// 设备是否已配对（存在于信任库中）。
    /// 「已信任设备自动接收」开关打开时，据此判断是否免确认自动接收。
    pub fn is_paired(&self, uuid: &str) -> bool {
        self.devices.lock().unwrap().contains_key(uuid)
    }

    /// 清除单条配对记录（设置页）。
    pub fn remove(&self, uuid: &str) -> LtResult<()> {
        let mut devices = self.devices.lock().unwrap();
        devices.remove(uuid);
        self.persist(&devices)
    }

    /// 清除全部配对记录。
    pub fn clear(&self) -> LtResult<()> {
        let mut devices = self.devices.lock().unwrap();
        devices.clear();
        self.persist(&devices)
    }

    /// 记录列表（设置页展示）。
    pub fn list(&self) -> Vec<TrustedDevice> {
        self.devices.lock().unwrap().values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_lifecycle() {
        let tmp = std::env::temp_dir().join(format!("lt-trust-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let store = TrustStore::load(&tmp).unwrap();
        assert_eq!(store.check("dev-1", "aa:bb"), TrustStatus::Unknown);

        store.add("dev-1", "AA:BB", "手机").unwrap();
        assert_eq!(store.check("dev-1", "aa:bb"), TrustStatus::Trusted);
        assert_eq!(store.check("dev-1", "cc:dd"), TrustStatus::Changed);

        // 持久化重载
        let store2 = TrustStore::load(&tmp).unwrap();
        assert_eq!(store2.check("dev-1", "aa:bb"), TrustStatus::Trusted);

        store2.set_auto_receive("dev-1", true).unwrap();
        assert!(store2.is_auto_receive("dev-1"));

        store2.remove("dev-1").unwrap();
        assert_eq!(store2.check("dev-1", "aa:bb"), TrustStatus::Unknown);
        let _ = fs::remove_dir_all(&tmp);
    }
}
