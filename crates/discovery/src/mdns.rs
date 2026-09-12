//! mDNS 通道（实施方案 6.1 主通道）。
//!
//! Windows 侧用 mdns-sd 组播 `_bolt._udp.local.`：
//! - 广播端（announcer）：注册本机服务，TXT 携带设备信息；
//! - 浏览端（browser）：发现/解析/移除事件 → 设备列表。
//!
//! TXT 属性：`uuid` / `dt`（设备类型）/ `name` / `qport` / `tport` / `ver` / `stealth`。
//! Android 侧不使用本模块（由 Kotlin NsdManager 桥接，见 [`crate::nsd_bridge`]）。

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::Duration;

use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::device::Device;
use crate::SERVICE_TYPE;

/// 本机广播参数。
#[derive(Debug, Clone)]
pub struct AnnounceInfo {
    pub uuid: String,
    pub name: String,
    pub device_type: u8,
    pub quic_port: u16,
    pub tcp_port: u16,
    pub proto_ver: u16,
    pub stealth: bool,
    /// 本机「传输协议」设置：true=只走 TCP（供对端拨号时取并集）
    pub prefer_tcp: bool,
}

/// mDNS 通道句柄（广播 + 浏览共用一个 daemon）。
pub struct MdnsChannel {
    daemon: ServiceDaemon,
    fullname: Option<String>,
}

impl MdnsChannel {
    pub fn new() -> std::io::Result<MdnsChannel> {
        let daemon = ServiceDaemon::new().map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(MdnsChannel {
            daemon,
            fullname: None,
        })
    }

    /// 注册本机服务（隐身模式下不调用）。
    ///
    /// 传入本机全部非环回 IPv4：mdns-sd 只在「注册地址属于该网卡」的接口上
    /// 广播（prepare_announce 按接口过滤 A 记录），多网卡主机必须注册全部
    /// 地址才能在各网段都被发现；单地址错误（如 127.0.0.1）会导致仅环回可见。
    pub fn announce(&mut self, info: &AnnounceInfo, ips: &[Ipv4Addr]) -> std::io::Result<()> {
        let instance = format!("bolt-{}", info.uuid);
        let host = format!("{}.local.", info.uuid.replace('-', ""));
        let dt = info.device_type.to_string();
        let qport = info.quic_port.to_string();
        let tport = info.tcp_port.to_string();
        let ver = info.proto_ver.to_string();
        let stealth = if info.stealth { "1" } else { "0" };
        let ptcp = if info.prefer_tcp { "1" } else { "0" };
        let props: &[(&str, &str)] = &[
            ("uuid", info.uuid.as_str()),
            ("dt", &dt),
            ("name", info.name.as_str()),
            ("qport", &qport),
            ("tport", &tport),
            ("ver", &ver),
            ("stealth", stealth),
            ("ptcp", ptcp),
        ];
        // AsIpAddrs 支持逗号分隔多地址
        let addrs = ips
            .iter()
            .map(|ip| ip.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let svc = ServiceInfo::new(
            SERVICE_TYPE,
            &instance,
            &host,
            addrs.as_str(),
            info.tcp_port,
            props,
        )
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?;
        self.daemon
            .register(svc)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        self.fullname = Some(format!("{instance}.{SERVICE_TYPE}"));
        Ok(())
    }

    /// 停止广播（注销服务）。
    pub fn stop_announce(&mut self) {
        if let Some(fullname) = self.fullname.take() {
            let _ = self.daemon.unregister(&fullname);
        }
    }

    /// 启动浏览：返回事件接收器，调用方起线程消费。
    pub fn browse(&self) -> std::io::Result<mdns_sd::Receiver<ServiceEvent>> {
        self.daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| std::io::Error::other(e.to_string()))
    }

    pub fn shutdown(self) {
        let _ = self.daemon.shutdown();
    }
}

/// 把一条已解析服务转成设备（缺少关键字段则返回 None）。
pub fn device_from_resolved(resolved: &ResolvedService, source: &str) -> Option<Device> {
    let uuid = resolved.get_property_val_str("uuid")?.to_string();
    if uuid.is_empty() {
        return None;
    }
    let name = resolved
        .get_property_val_str("name")
        .unwrap_or("unknown")
        .to_string();
    let device_type: u8 = resolved
        .get_property_val_str("dt")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let qport: u16 = resolved
        .get_property_val_str("qport")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let proto_ver: u16 = resolved
        .get_property_val_str("ver")
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    let stealth = resolved.get_property_val_str("stealth") == Some("1");
    let prefer_tcp = resolved.get_property_val_str("ptcp") == Some("1");

    let addrs = resolved.get_addresses_v4();
    let ip = addrs
        .iter()
        .find(|ip| !ip.is_loopback() && !ip.is_link_local())
        .or_else(|| addrs.iter().next())
        .copied()?;

    Some(Device {
        uuid,
        name,
        device_type,
        ip: ip.to_string(),
        quic_port: if qport != 0 {
            qport
        } else {
            resolved.get_port()
        },
        tcp_port: resolved.get_port(),
        proto_ver,
        stealth,
        prefer_tcp,
        source: source.to_string(),
        last_seen_unix: Device::now_unix(),
    })
}

/// 浏览消费循环（阻塞，适合独立线程）。
///
/// 0.21 的 `ServiceRemoved` 只带 (service_type, fullname)，故本地维护
/// fullname→uuid 映射来定位被移除的设备。
pub fn browse_loop(
    receiver: mdns_sd::Receiver<ServiceEvent>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    mut on_upsert: impl FnMut(Device),
    mut on_remove: impl FnMut(String),
) {
    let mut by_fullname: HashMap<String, String> = HashMap::new();
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        let Ok(event) = receiver.recv_timeout(Duration::from_millis(500)) else {
            continue; // daemon 仍在，继续等待
        };
        match event {
            ServiceEvent::ServiceResolved(resolved) => {
                if let Some(dev) = device_from_resolved(&resolved, "mdns") {
                    by_fullname.insert(resolved.get_fullname().to_string(), dev.uuid.clone());
                    on_upsert(dev);
                }
            }
            ServiceEvent::ServiceRemoved(_ty, fullname) => {
                if let Some(uuid) = by_fullname.remove(&fullname) {
                    on_remove(uuid);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved_fixture(with_uuid: bool) -> ResolvedService {
        let uuid = if with_uuid { "abc-123" } else { "" };
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            "bolt-test",
            "test.local.",
            std::net::IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50)),
            8901,
            &[
                ("uuid", uuid),
                ("dt", "1"),
                ("name", "我的电脑"),
                ("qport", "8899"),
                ("tport", "8901"),
                ("ver", "2"),
                ("stealth", "0"),
                ("ptcp", "1"),
            ][..],
        )
        .unwrap();
        info.as_resolved_service()
    }

    #[test]
    fn txt_roundtrip_fields() {
        let resolved = resolved_fixture(true);
        let dev = device_from_resolved(&resolved, "mdns").expect("parse");
        assert_eq!(dev.uuid, "abc-123");
        assert_eq!(dev.name, "我的电脑");
        assert_eq!(dev.quic_port, 8899);
        assert_eq!(dev.tcp_port, 8901);
        assert_eq!(dev.ip, "192.168.1.50");
        assert!(!dev.stealth);
        assert!(dev.prefer_tcp);
    }

    #[test]
    fn missing_uuid_rejected() {
        let resolved = resolved_fixture(false);
        assert!(device_from_resolved(&resolved, "mdns").is_none());
    }
}
