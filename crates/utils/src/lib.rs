//! utils：LocalTransfer 通用工具层。
//!
//! 提供统一错误码（-15~0，见实施方案第十二节）、应用配置、
//! 设备 UUID 生成与端口池探测等基础能力。所有上层 crate 均依赖本层。

pub mod config;
pub mod constants;
pub mod error;
pub mod id;
pub mod net;

pub use config::AppConfig;
pub use constants::*;
pub use error::{LtError, LtResult};

/// 默认监听端口（UDP + TCP），占用时自动向上尝试到 8950。
pub const DEFAULT_PORT: u16 = 8899;
/// 端口池上限（含）。
pub const MAX_PORT: u16 = 8950;
/// UDP 广播探测端口（探测报文目标端口）。
///
/// 必须独立于 QUIC/TCP 端口池（8899~8950）：早期与 DEFAULT_PORT 同为 8899，
/// 而引擎先于发现启动、QUIC 先绑定 8899/udp，导致两端探测监听都退化到临时
/// 端口，广播探测包落在对端 QUIC 端点上被丢弃——探测通道双向全废，设备列表
/// 失去持续刷新、条目被 TTL 清扫（表现为「连上后对方设备很快消失」）。
pub const UDP_PROBE_PORT: u16 = 8951;
/// 协议版本（协议规范 V2）。
pub const PROTOCOL_VERSION: u16 = 2;
/// 产品标识，用于 mDNS 实例名/日志。
pub const PRODUCT_NAME: &str = "LocalTransfer";
