//! discovery：设备发现（三通道，实施方案第六节）。
//!
//! - [`mdns`] mDNS 通道（Windows 侧 mdns-sd 组播 `_lt._udp.local.`）；
//! - [`udp_probe`] UDP 广播探测兜底通道（255.255.255.255，独立端口 [`PROBE_PORT`]）；
//! - [`device_list`] 设备列表维护（UUID 去重、10s 过期移除）；
//! - [`nsd_bridge`] Android NsdManager 结果注入桥（JNI 回调接入）；
//! - [`manager`] 发现管理器（隐身模式统一开关）。

pub mod device;
pub mod device_list;
pub mod manager;
pub mod mdns;
pub mod nsd_bridge;
pub mod udp_probe;

pub use device::Device;
pub use device_list::DeviceList;
pub use manager::{DiscoveryConfig, DiscoveryManager};

/// mDNS 服务类型。
pub const SERVICE_TYPE: &str = "_lt._udp.local.";
/// UDP 探测广播目标端口（独立于 QUIC 端口池，见 [`utils::UDP_PROBE_PORT`]）。
pub const PROBE_PORT: u16 = utils::UDP_PROBE_PORT;
