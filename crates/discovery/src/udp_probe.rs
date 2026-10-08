//! UDP 广播探测兜底通道（实施方案 6.2）。
//!
//! 网络封锁组播 / mDNS 不可用时兜底：
//! - 主动端每 3s 向 `255.255.255.255:PROBE_PORT` 广播查询包 `BTQ1`；
//! - 被动端监听 `PROBE_PORT/udp`，收到查询后单播回复 `BTP1` + 本机设备信息 JSON；
//! - 双方都把对方刷新进设备列表（UUID 去重，兼作设备列表 TTL 的持续刷新源）。
//!
//! 端口独立于 QUIC/TCP 端口池（见 [`crate::PROBE_PORT`] 注释）；监听仍用
//! SO_REUSEADDR 尽力绑定，失败则退化到临时端口（此场景下本通道单向降级，
//! 靠 mDNS 主通道兜底）。

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};

use crate::device::Device;
use crate::PROBE_PORT;

const QUERY_MAGIC: &[u8; 4] = b"BTQ1";
const QUERY_WITH_DEV_MAGIC: &[u8; 4] = b"BTQ2";
const PREPLY_MAGIC: &[u8; 4] = b"BTP1";

/// 构造带 SO_REUSEADDR 的 UDP socket，尽力绑定指定端口。
fn reuse_socket(port: u16) -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
    match socket.bind(&bind_addr.into()) {
        Ok(()) => Ok(socket.into()),
        Err(e) => {
            tracing::warn!(port, err = %e, "UDP 探测端口占用，退化到临时端口（探测通道降级）");
            let fallback = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
            socket.bind(&fallback.into())?;
            Ok(socket.into())
        }
    }
}

/// 构造绑定到指定本地 IP 的发送 socket（广播出口跟随绑定地址所属网卡）。
fn iface_sender(ip: Ipv4Addr) -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.set_broadcast(true)?;
    socket.bind(&SocketAddrV4::new(ip, 0).into())?;
    Ok(socket.into())
}

/// 单个网卡接口的发送配置与套接字。
#[derive(Clone)]
pub struct InterfaceSender {
    pub ip: Ipv4Addr,
    pub mask: Ipv4Addr,
    pub bcast_addr: Ipv4Addr,
    pub socket: Arc<UdpSocket>,
}

/// 探测通道（监听 + 逐网卡定向/全局广播发送 + 动态网卡感知自愈）。
pub struct UdpProbe {
    socket: Arc<UdpSocket>,
    senders: Arc<std::sync::RwLock<Vec<InterfaceSender>>>,
}

impl UdpProbe {
    /// 创建探测通道：监听尽力绑定 `PROBE_PORT/udp`；
    /// 初始网卡集合经 `local_nets` 初始化，支持运行时热更新。
    pub fn new(local_nets: &[(Ipv4Addr, Ipv4Addr)]) -> std::io::Result<UdpProbe> {
        let socket = reuse_socket(PROBE_PORT)?;
        socket.set_broadcast(true)?;
        let probe = UdpProbe {
            socket: Arc::new(socket),
            senders: Arc::new(std::sync::RwLock::new(Vec::new())),
        };
        probe.update_interfaces(local_nets);
        Ok(probe)
    }

    /// 动态感知并增量更新网卡接口（连上热点/Wi-Fi/断开连接自愈）。
    pub fn update_interfaces(&self, nets: &[(Ipv4Addr, Ipv4Addr)]) {
        let valid_nets: Vec<(Ipv4Addr, Ipv4Addr)> = nets
            .iter()
            .filter(|(ip, _)| !ip.is_loopback() && !ip.is_link_local())
            .copied()
            .collect();

        let mut current = self.senders.write().unwrap();
        let is_same = current.len() == valid_nets.len()
            && valid_nets.iter().all(|(ip, mask)| {
                current.iter().any(|s| s.ip == *ip && s.mask == *mask)
            });
        if is_same {
            return;
        }

        let mut updated = Vec::new();
        for (ip, mask) in valid_nets {
            let bcast_addr = utils::net::broadcast_addr(ip, mask);
            if let Some(existing) = current.iter().find(|s| s.ip == ip) {
                updated.push(InterfaceSender {
                    ip,
                    mask,
                    bcast_addr,
                    socket: existing.socket.clone(),
                });
            } else {
                match iface_sender(ip) {
                    Ok(sock) => {
                        tracing::info!(%ip, %bcast_addr, "UDP 探测发现新网卡，绑定广播发送套接字");
                        updated.push(InterfaceSender {
                            ip,
                            mask,
                            bcast_addr,
                            socket: Arc::new(sock),
                        });
                    }
                    Err(e) => {
                        tracing::warn!(%ip, err = %e, "新网卡 UDP 探测套接字创建失败");
                    }
                }
            }
        }
        *current = updated;
    }

    /// 绑定到的实际本地端口（退化时可能不是 `PROBE_PORT`）。
    pub fn local_port(&self) -> u16 {
        self.socket.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    /// 在每个网卡上广播一次查询。
    /// - 发往各网卡子网定向广播（Direct Broadcast，在 Wi-Fi 热点及 AP 隔离下穿透极强）；
    /// - 同时发往受限全局广播 255.255.255.255；
    /// - 支持发送携带本机 Device JSON 的 BTQ2 报文，对端单包即可互通；同时发轻量 BTQ1 兼容旧版本。
    pub fn send_probe(&self, own_uuid: &str, me: Option<&Device>) -> std::io::Result<()> {
        let mut btq1 = Vec::with_capacity(4 + own_uuid.len());
        btq1.extend_from_slice(QUERY_MAGIC);
        btq1.extend_from_slice(own_uuid.as_bytes());

        let global_target = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::BROADCAST, PROBE_PORT));
        let senders = self.senders.read().unwrap().clone();
        let mut last = Ok(());

        if senders.is_empty() {
            let _ = self.socket.send_to(&btq1, global_target);
            if let Some(dev) = me {
                if let Ok(payload) = serde_json::to_vec(dev) {
                    let mut b = Vec::with_capacity(4 + payload.len());
                    b.extend_from_slice(QUERY_WITH_DEV_MAGIC);
                    b.extend_from_slice(&payload);
                    let _ = self.socket.send_to(&b, global_target);
                }
            }
        } else {
            // 对每个物理网卡：仅向该网卡的子网定向广播发送 1 次探测（优先包含 Device 信息的 BTQ2，避免广播冗余）
            for s in &senders {
                let subnet_target = SocketAddr::V4(SocketAddrV4::new(s.bcast_addr, PROBE_PORT));
                let mut sent_subnet = false;
                if let Some(dev) = me {
                    let mut dev_clone = dev.clone();
                    dev_clone.ip = s.ip.to_string();
                    if let Ok(payload) = serde_json::to_vec(&dev_clone) {
                        let mut b = Vec::with_capacity(4 + payload.len());
                        b.extend_from_slice(QUERY_WITH_DEV_MAGIC);
                        b.extend_from_slice(&payload);
                        let _ = s.socket.send_to(&b, subnet_target);
                        sent_subnet = true;
                    }
                }
                if !sent_subnet {
                    let _ = s.socket.send_to(&btq1, subnet_target);
                }
            }

            // 全局受限广播 255.255.255.255 仅作为单次全局兜底，无需对每张网卡重复打满
            if let Err(e) = self.socket.send_to(&btq1, global_target) {
                last = Err(e);
            }
        }
        last
    }

    fn handle_packet(
        sock: &UdpSocket,
        data: &[u8],
        from: SocketAddr,
        own_uuid: &str,
        reply: &mut impl FnMut(SocketAddr) -> Option<Device>,
        on_device: &mut impl FnMut(Device),
    ) {
        if data.len() >= 4 && &data[..4] == QUERY_MAGIC {
            let sender_uuid = String::from_utf8_lossy(&data[4..]).to_string();
            if sender_uuid == own_uuid {
                return;
            }
            if let Some(me) = reply(from) {
                if let Ok(payload) = serde_json::to_vec(&me) {
                    let mut out = Vec::with_capacity(4 + payload.len());
                    out.extend_from_slice(PREPLY_MAGIC);
                    out.extend_from_slice(&payload);
                    let _ = sock.send_to(&out, from);
                }
            }
        } else if data.len() > 4 && &data[..4] == QUERY_WITH_DEV_MAGIC {
            if let Ok(mut dev) = serde_json::from_slice::<Device>(&data[4..]) {
                if dev.uuid != own_uuid {
                    // 物理网络来源 IP 强制锚定：以数据包物理抵达接口的真实来源为准，杜绝任何假 IP 和跳变
                    dev.ip = from.ip().to_string();
                    on_device(dev);
                    if let Some(me) = reply(from) {
                        if let Ok(payload) = serde_json::to_vec(&me) {
                            let mut out = Vec::with_capacity(4 + payload.len());
                            out.extend_from_slice(PREPLY_MAGIC);
                            out.extend_from_slice(&payload);
                            let _ = sock.send_to(&out, from);
                        }
                    }
                }
            }
        } else if data.len() > 4 && &data[..4] == PREPLY_MAGIC {
            if let Ok(mut dev) = serde_json::from_slice::<Device>(&data[4..]) {
                if dev.uuid != own_uuid {
                    dev.ip = from.ip().to_string();
                    on_device(dev);
                }
            }
        }
    }

    /// 阻塞监听循环（适合独立线程）：
    /// - 收到查询（非自己）→ 用 `reply(from)` 单播回复本机信息；
    /// - 收到回复或 BTQ2 广播 → 解析为设备 `on_device` 入列。
    pub fn listen_loop(
        &self,
        own_uuid: String,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        mut reply: impl FnMut(std::net::SocketAddr) -> Option<Device>,
        mut on_device: impl FnMut(Device),
    ) {
        let mut buf = [0u8; 4096];
        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
            let mut received = false;
            let active_socks: Vec<Arc<UdpSocket>> = {
                let senders = self.senders.read().unwrap();
                senders.iter().map(|s| s.socket.clone()).collect()
            };

            // 1. 读主监听 socket (0.0.0.0:PROBE_PORT)
            loop {
                match self.socket.recv_from(&mut buf) {
                    Ok((len, from)) => {
                        received = true;
                        Self::handle_packet(
                            &self.socket,
                            &buf[..len],
                            from,
                            &own_uuid,
                            &mut reply,
                            &mut on_device,
                        );
                    }
                    Err(ref e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        break;
                    }
                    Err(_) => return,
                }
            }

            // 2. 读各网卡发送套接字（接收定向单播应答）
            for sock in &active_socks {
                loop {
                    match sock.recv_from(&mut buf) {
                        Ok((len, from)) => {
                            received = true;
                            Self::handle_packet(
                                sock,
                                &buf[..len],
                                from,
                                &own_uuid,
                                &mut reply,
                                &mut on_device,
                            );
                        }
                        Err(ref e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::TimedOut =>
                            {
                                break;
                            }
                        Err(_) => break,
                    }
                }
            }

            if !received {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reply_roundtrip_local() {
        // 两个临时端口套接字直接互发（不走 255.255.255.255，避免测试环境限制）
        let a_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let b_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let b_addr = b_socket.local_addr().unwrap();

        // A 发查询
        let mut q = Vec::new();
        q.extend_from_slice(QUERY_MAGIC);
        q.extend_from_slice(b"uuid-a");
        a_socket.send_to(&q, b_addr).unwrap();

        // B 收查询并回复
        let mut buf = [0u8; 64];
        let (len, from) = b_socket.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..4], QUERY_MAGIC);
        assert_eq!(&buf[4..len], b"uuid-a");

        let dev = Device {
            uuid: "uuid-b".into(),
            name: "B".into(),
            device_type: 1,
            ip: "127.0.0.1".into(),
            quic_port: 8899,
            tcp_port: 8899,
            proto_ver: 2,
            stealth: false,
            prefer_tcp: false,
            source: "udp_probe".into(),
            last_seen_unix: Device::now_unix(),
        };
        let payload = serde_json::to_vec(&dev).unwrap();
        let mut out = Vec::new();
        out.extend_from_slice(PREPLY_MAGIC);
        out.extend_from_slice(&payload);
        b_socket.send_to(&out, from).unwrap();

        // A 收回复
        let mut buf2 = [0u8; 2048];
        let (len2, _) = a_socket.recv_from(&mut buf2).unwrap();
        assert_eq!(&buf2[..4], PREPLY_MAGIC);
        let got: Device = serde_json::from_slice(&buf2[4..len2]).unwrap();
        assert_eq!(got.uuid, "uuid-b");
    }

    #[test]
    fn probe_btq2_roundtrip_local() {
        let a_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let b_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let b_addr = b_socket.local_addr().unwrap();

        let a_dev = Device {
            uuid: "uuid-a".into(),
            name: "Device A".into(),
            device_type: 1,
            ip: "127.0.0.1".into(),
            quic_port: 8899,
            tcp_port: 8899,
            proto_ver: 2,
            stealth: false,
            prefer_tcp: false,
            source: "udp_probe".into(),
            last_seen_unix: Device::now_unix(),
        };

        // A 发送 BTQ2 报文（直接携带本机设备 JSON）
        let mut q2 = Vec::new();
        q2.extend_from_slice(QUERY_WITH_DEV_MAGIC);
        q2.extend_from_slice(&serde_json::to_vec(&a_dev).unwrap());
        a_socket.send_to(&q2, b_addr).unwrap();

        // B 接收 BTQ2 报文并直接解析出 A
        let mut buf = [0u8; 2048];
        let (len, from) = b_socket.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..4], QUERY_WITH_DEV_MAGIC);
        let got_a: Device = serde_json::from_slice(&buf[4..len]).unwrap();
        assert_eq!(got_a.uuid, "uuid-a");
        assert_eq!(got_a.name, "Device A");

        // B 触发回复 BTP1 给 A
        let b_dev = Device {
            uuid: "uuid-b".into(),
            name: "Device B".into(),
            device_type: 2,
            ip: "127.0.0.1".into(),
            quic_port: 9000,
            tcp_port: 9000,
            proto_ver: 2,
            stealth: false,
            prefer_tcp: false,
            source: "udp_probe".into(),
            last_seen_unix: Device::now_unix(),
        };
        let mut out = Vec::new();
        out.extend_from_slice(PREPLY_MAGIC);
        out.extend_from_slice(&serde_json::to_vec(&b_dev).unwrap());
        b_socket.send_to(&out, from).unwrap();

        // A 接收 B 的单播应答
        let mut buf2 = [0u8; 2048];
        let (len2, _) = a_socket.recv_from(&mut buf2).unwrap();
        assert_eq!(&buf2[..4], PREPLY_MAGIC);
        let got_b: Device = serde_json::from_slice(&buf2[4..len2]).unwrap();
        assert_eq!(got_b.uuid, "uuid-b");
    }
}
