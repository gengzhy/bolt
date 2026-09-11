//! Android NsdManager 桥（实施方案 6.3）。
//!
//! Android 上 mDNS 由 Kotlin 侧 `NsdManager` 完成（系统服务，免组播权限），
//! 发现结果经 JNI 注入本桥 → 统一进设备列表。Rust 侧不做发现，只做汇聚。

use std::sync::Arc;

use crate::device::Device;
use crate::device_list::DeviceList;

/// NsdManager 结果注入桥。
pub struct NsdBridge {
    devices: Arc<DeviceList>,
}

impl NsdBridge {
    pub fn new(devices: Arc<DeviceList>) -> NsdBridge {
        NsdBridge { devices }
    }

    /// 注入一条发现结果（Kotlin 侧 JSON：字段与 [`Device`] 对齐）。
    /// 缺少必填字段（uuid/ip）时忽略并返回 false。
    pub fn inject_json(&self, json: &str) -> bool {
        #[derive(serde::Deserialize)]
        struct In {
            uuid: String,
            ip: String,
            #[serde(default)]
            name: String,
            #[serde(default)]
            dt: u8,
            #[serde(default)]
            qport: u16,
            #[serde(default)]
            tport: u16,
            #[serde(default = "default_ver")]
            ver: u16,
            #[serde(default)]
            stealth: bool,
            #[serde(default)]
            ptcp: bool,
        }
        fn default_ver() -> u16 {
            2
        }
        let Ok(d) = serde_json::from_str::<In>(json) else {
            tracing::warn!("NSD 注入 JSON 解析失败");
            return false;
        };
        if d.uuid.is_empty() || d.ip.is_empty() {
            return false;
        }
        self.devices.upsert(Device {
            uuid: d.uuid,
            name: d.name,
            device_type: d.dt,
            ip: d.ip,
            quic_port: d.qport,
            tcp_port: if d.tport != 0 { d.tport } else { d.qport },
            proto_ver: d.ver,
            stealth: d.stealth,
            prefer_tcp: d.ptcp,
            source: "nsd".into(),
            last_seen_unix: Device::now_unix(),
        })
    }

    /// Kotlin 侧服务丢失回调。
    pub fn remove(&self, uuid: &str) {
        self.devices.remove(uuid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_and_remove() {
        let list = Arc::new(DeviceList::new());
        let bridge = NsdBridge::new(list.clone());
        assert!(bridge
            .inject_json(r#"{"uuid":"a1","ip":"192.168.1.20","name":"安卓","dt":2,"qport":8899}"#));
        assert_eq!(list.snapshot().len(), 1);
        let dev = &list.snapshot()[0];
        assert_eq!(dev.tcp_port, 8899); // 缺省回落
        assert!(!bridge.inject_json(r#"{"ip":"1.2.3.4"}"#)); // 缺 uuid
        assert!(!bridge.inject_json("{bad json"));
        bridge.remove("a1");
        assert!(list.snapshot().is_empty());
    }
}
