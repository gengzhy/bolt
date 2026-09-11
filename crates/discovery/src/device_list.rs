//! 设备列表维护：UUID 去重、10s 未刷新移除、变更通知（实施方案 6.1）。

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Mutex;

use crate::device::Device;

type ChangeCallback = Box<dyn Fn(&[Device]) + Send + Sync>;

/// 线程安全设备列表（由三通道 + 手动注入共同维护）。
pub struct DeviceList {
    map: Mutex<HashMap<String, Device>>,
    on_change: Mutex<Option<ChangeCallback>>,
    /// 本机 UUID：发现通道会看到自己注册的服务，入列时过滤
    own_uuid: String,
    /// 存在活跃连接的设备：过期清扫时保留（连接中的对端不应从列表消失）
    pinned: Mutex<HashSet<String>>,
}

impl Default for DeviceList {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceList {
    pub fn new() -> DeviceList {
        DeviceList::with_own_uuid(String::new())
    }

    /// `own_uuid` 非空时，upsert 忽略与本机同 UUID 的发现结果。
    pub fn with_own_uuid(own_uuid: String) -> DeviceList {
        DeviceList {
            map: Mutex::new(HashMap::new()),
            on_change: Mutex::new(None),
            own_uuid,
            pinned: Mutex::new(HashSet::new()),
        }
    }

    pub fn set_change_callback(&self, cb: impl Fn(&[Device]) + Send + Sync + 'static) {
        *self.on_change.lock().unwrap() = Some(Box::new(cb));
        self.notify();
    }

    /// 插入或刷新设备（以 uuid 去重）。返回是否变化。
    pub fn upsert(&self, device: Device) -> bool {
        if !self.own_uuid.is_empty() && device.uuid == self.own_uuid {
            return false; // 本机自己的广播/注册，不进设备列表
        }
        let mut map = self.map.lock().unwrap();
        let changed = match map.get(&device.uuid) {
            None => true,
            Some(old) => {
                old.ip != device.ip
                    || old.quic_port != device.quic_port
                    || old.name != device.name
                    || old.prefer_tcp != device.prefer_tcp
            }
        };
        map.insert(device.uuid.clone(), device);
        drop(map);
        if changed {
            self.notify();
        }
        changed
    }

    pub fn remove(&self, uuid: &str) {
        let removed = self.map.lock().unwrap().remove(uuid).is_some();
        if removed {
            self.notify();
        }
    }

    /// 移除 TTL 过期条目（10s 未刷新）；[`Self::pin`] 钉住的设备保留。
    pub fn sweep_expired(&self, ttl_secs: u64) {
        let now = Device::now_unix();
        let pinned = self.pinned.lock().unwrap().clone();
        let mut map = self.map.lock().unwrap();
        let before = map.len();
        map.retain(|uuid, d| {
            pinned.contains(uuid) || now.saturating_sub(d.last_seen_unix) < ttl_secs
        });
        let changed = map.len() != before;
        drop(map);
        if changed {
            self.notify();
        }
    }

    /// 钉住设备（存在活跃连接）：过期清扫不移除。
    pub fn pin(&self, uuid: &str) {
        self.pinned.lock().unwrap().insert(uuid.to_string());
    }

    /// 解除钉住（连接断开后恢复 TTL 清扫）。
    pub fn unpin(&self, uuid: &str) {
        self.pinned.lock().unwrap().remove(uuid);
    }

    /// 快照（JSON 推送与 FFI 查询共用）。
    pub fn snapshot(&self) -> Vec<Device> {
        self.map.lock().unwrap().values().cloned().collect()
    }

    pub fn snapshot_json(&self) -> String {
        serde_json::to_string(&self.snapshot()).unwrap_or_else(|_| "[]".into())
    }

    pub fn clear(&self) {
        self.map.lock().unwrap().clear();
        self.notify();
    }

    fn notify(&self) {
        let devices = self.snapshot();
        if let Some(cb) = self.on_change.lock().unwrap().as_ref() {
            cb(&devices);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn dev(uuid: &str, ip: &str) -> Device {
        Device {
            uuid: uuid.into(),
            name: "d".into(),
            device_type: 1,
            ip: ip.into(),
            quic_port: 8899,
            tcp_port: 8899,
            proto_ver: 2,
            stealth: false,
            prefer_tcp: false,
            source: "test".into(),
            last_seen_unix: Device::now_unix(),
        }
    }

    #[test]
    fn dedup_and_change() {
        let list = DeviceList::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c2 = count.clone();
        list.set_change_callback(move |_| {
            c2.fetch_add(1, Ordering::SeqCst);
        });
        // set_change_callback 触发一次
        assert!(list.upsert(dev("u1", "1.1.1.1")));
        assert!(!list.upsert(dev("u1", "1.1.1.1"))); // 无变化
        assert!(list.upsert(dev("u1", "2.2.2.2"))); // IP 变化
        assert!(list.upsert(dev("u2", "3.3.3.3")));
        assert_eq!(list.snapshot().len(), 2);
        assert_eq!(count.load(Ordering::SeqCst), 4); // 1(set) + 3(变化)
    }

    #[test]
    fn sweep_expired() {
        let list = DeviceList::new();
        let mut d = dev("u1", "1.1.1.1");
        d.last_seen_unix = Device::now_unix().saturating_sub(60);
        list.upsert(d);
        list.sweep_expired(10);
        assert!(list.snapshot().is_empty());
    }
}
