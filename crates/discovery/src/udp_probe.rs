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
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};

use crate::device::Device;
use crate::PROBE_PORT;

const QUERY_MAGIC: &[u8; 4] = b"BTQ1";
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

/// 探测通道（监听 + 逐网卡广播发送）。
///
/// 发送必须逐网卡进行：`255.255.255.255` 只从默认路由网卡出口，多网卡主机
/// 上探测包会发往错误子网（对端永远收不到）。绑定每个非环回本地 IP 的
/// socket 各发一次广播，保证探测到达全部子网。
pub struct UdpProbe {
    socket: UdpSocket,
    senders: Vec<UdpSocket>,
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

impl UdpProbe {
    /// 创建探测通道：监听尽力绑定 `PROBE_PORT/udp`；
    /// 发送按 `local_ips` 逐网卡广播（空则退化默认路由 socket）。
    pub fn new(local_ips: &[Ipv4Addr]) -> std::io::Result<UdpProbe> {
        let socket = reuse_socket(PROBE_PORT)?;
        socket.set_broadcast(true)?;
        let mut senders = Vec::new();
        for ip in local_ips {
            if ip.is_loopback() {
                continue;
            }
            match iface_sender(*ip) {
                Ok(s) => senders.push(s),
                Err(e) => tracing::warn!(%ip, err = %e, "探测网卡发送套接字创建失败"),
            }
        }
        if senders.is_empty() {
            senders.push(socket.try_clone()?);
        }
        Ok(UdpProbe { socket, senders })
    }

    /// 绑定到的实际本地端口（退化时可能不是 `PROBE_PORT`）。
    pub fn local_port(&self) -> u16 {
        self.socket.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    /// 在每个网卡上广播一次查询。`own_uuid` 用于让监听方过滤自己。
    pub fn send_probe(&self, own_uuid: &str) -> std::io::Result<()> {
        let mut buf = Vec::with_capacity(4 + own_uuid.len());
        buf.extend_from_slice(QUERY_MAGIC);
        buf.extend_from_slice(own_uuid.as_bytes());
        let target = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::BROADCAST, PROBE_PORT));
        let mut last = Ok(());
        for s in &self.senders {
            if let Err(e) = s.send_to(&buf, target) {
                last = Err(e);
            }
        }
        last
    }

    /// 阻塞监听循环（适合独立线程）：
    /// - 收到查询（非自己）→ 用 `reply(from)` 单播回复本机信息（`from` 为对端地址，
    ///   供多网卡主机按对端子网挑选应答 IP）；
    /// - 收到回复 → 解析为设备 `on_device` 入列。
    ///
    /// 需轮询全部套接字：应答单播回「查询的源端口」，即逐网卡发送套接字
    /// （临时端口），只监听主 socket 会丢应答。
    pub fn listen_loop(
        &self,
        own_uuid: String,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        mut reply: impl FnMut(std::net::SocketAddr) -> Option<Device>,
        mut on_device: impl FnMut(Device),
    ) {
        let mut buf = [0u8; 2048];
        let socks: Vec<&UdpSocket> = std::iter::once(&self.socket)
            .chain(self.senders.iter())
            .collect();
        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
            let mut received = false;
            for sock in &socks {
                loop {
                    match sock.recv_from(&mut buf) {
                        Ok((len, from)) => {
                            received = true;
                            let data = &buf[..len];
                            if data.len() >= 4 && &data[..4] == QUERY_MAGIC {
                                let sender_uuid = String::from_utf8_lossy(&data[4..]).to_string();
                                if sender_uuid == own_uuid {
                                    continue;
                                }
                                if let Some(me) = reply(from) {
                                    if let Ok(payload) = serde_json::to_vec(&me) {
                                        let mut out = Vec::with_capacity(4 + payload.len());
                                        out.extend_from_slice(PREPLY_MAGIC);
                                        out.extend_from_slice(&payload);
                                        let _ = sock.send_to(&out, from);
                                    }
                                }
                            } else if data.len() > 4 && &data[..4] == PREPLY_MAGIC {
                                if let Ok(dev) = serde_json::from_slice::<Device>(&data[4..]) {
                                    if dev.uuid != own_uuid {
                                        on_device(dev);
                                    }
                                }
                            }
                        }
                        Err(ref e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::TimedOut =>
                        {
                            break; // 该套接字暂空，看下一个
                        }
                        Err(_) => return,
                    }
                }
            }
            if !received {
                std::thread::sleep(Duration::from_millis(100));
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
}
