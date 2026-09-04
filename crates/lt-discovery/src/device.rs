//! 设备条目（发现结果的统一表示）。

use serde::{Deserialize, Serialize};

/// 局域网内发现的一台设备。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub uuid: String,
    pub name: String,
    /// 设备类型（1 Windows / 2 Android / …）
    pub device_type: u8,
    pub ip: String,
    pub quic_port: u16,
    pub tcp_port: u16,
    pub proto_ver: u16,
    /// 隐身标志（仍可直连，只是对方主动隐藏）
    #[serde(default)]
    pub stealth: bool,
    /// 对端「传输协议」设置：true=该设备只走 TCP（二选一模式下）。
    /// 拨号方取「本机设置 ∨ 对端设置」的并集——任一端选了 TCP 就用 TCP，
    /// 避免「改了设置却不生效」（谁拨号谁说了算导致另一端设置被忽略）。
    #[serde(default)]
    pub prefer_tcp: bool,
    /// 发现来源：mdns / udp_probe / nsd / manual / conn（连接 HELLO 校正写入）
    pub source: String,
    /// 最近一次刷新（unix 秒）
    pub last_seen_unix: u64,
}

impl Device {
    pub fn now_unix() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}
