//! IP / 端口工具：端口池探测（8899~8950）、本机地址枚举。

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, UdpSocket};

use crate::error::{LtError, LtResult};
use crate::{DEFAULT_PORT, MAX_PORT};

/// 在端口池内寻找第一个可同时绑定 TCP 与 UDP 的端口。
///
/// 返回实际端口；若端口池耗尽返回 `LtError::PortUnavailable`。
pub fn find_available_port(preferred: u16, bind_addr: IpAddr) -> LtResult<u16> {
    let start = if preferred == 0 {
        DEFAULT_PORT
    } else {
        preferred
    };
    let candidates: Vec<u16> = (start..=MAX_PORT).chain(DEFAULT_PORT..start).collect();
    for port in candidates {
        let addr = SocketAddr::new(bind_addr, port);
        if TcpListener::bind(addr).is_ok() && UdpSocket::bind(addr).is_ok() {
            return Ok(port);
        }
    }
    Err(LtError::PortUnavailable)
}

/// 判断 TCP 端口当前是否可绑定。
pub fn tcp_port_free(addr: SocketAddr) -> bool {
    TcpListener::bind(addr).is_ok()
}

/// 枚举本机非环回 IPv4 地址及子网掩码（多网卡主机按接口分别广播）。
///
/// 返回 `(地址, 掩码)` 列表；网卡枚举失败时退化为组播路由探测单地址。
pub fn local_ipv4_nets() -> Vec<(Ipv4Addr, Ipv4Addr)> {
    let mut out = Vec::new();
    if let Ok(ifs) = if_addrs::get_if_addrs() {
        for i in ifs {
            if let if_addrs::IfAddr::V4(v4) = i.addr {
                // 跳过环回与 169.254 链路本地地址（不可路由到对端）
                if !v4.ip.is_loopback() && !v4.ip.is_link_local() {
                    out.push((v4.ip, v4.netmask));
                }
            }
        }
    }
    if out.is_empty() {
        if let Some(ip) = probe_egress_ipv4() {
            out.push((ip, Ipv4Addr::new(255, 255, 255, 0)));
        }
    }
    out
}

/// 枚举本机非环回 IPv4 地址（用于服务发现展示与防火墙提示）。
pub fn local_ipv4s() -> Vec<Ipv4Addr> {
    local_ipv4_nets().into_iter().map(|(ip, _)| ip).collect()
}

/// 多网卡场景挑选与对端同子网的本地地址；无同子网地址时取第一个。
pub fn pick_ip_for_peer(nets: &[(Ipv4Addr, Ipv4Addr)], peer: IpAddr) -> Option<Ipv4Addr> {
    let fallback = nets.first().map(|(ip, _)| *ip);
    let IpAddr::V4(p) = peer else {
        return fallback;
    };
    nets.iter()
        .find(|(ip, mask)| in_same_subnet(*ip, p, *mask))
        .map(|(ip, _)| *ip)
        .or(fallback)
}

fn in_same_subnet(a: Ipv4Addr, b: Ipv4Addr, mask: Ipv4Addr) -> bool {
    (u32::from(a) & u32::from(mask)) == (u32::from(b) & u32::from(mask))
}

/// 组播路由探测出口地址（网卡枚举不可用时的兜底，不真正发包）。
fn probe_egress_ipv4() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("224.0.0.251:5353").ok()?;
    let addr = sock.local_addr().ok()?;
    match addr.ip() {
        IpAddr::V4(v4) if !v4.is_loopback() => Some(v4),
        _ => None,
    }
}

/// 解析 "IP" 或 "IP:port" 形式的目标地址（手动 IP 直连入口）。
pub fn parse_target(target: &str, default_port: u16) -> LtResult<SocketAddr> {
    if let Ok(addr) = target.parse::<SocketAddr>() {
        return Ok(addr);
    }
    if let Ok(ip) = target.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, default_port));
    }
    Err(LtError::InvalidArgument)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_targets() {
        assert_eq!(
            parse_target("192.168.1.5", 8899).unwrap(),
            "192.168.1.5:8899".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            parse_target("192.168.1.5:9000", 8899).unwrap(),
            "192.168.1.5:9000".parse::<SocketAddr>().unwrap()
        );
        assert!(parse_target("not-an-ip", 8899).is_err());
    }

    #[test]
    fn finds_port_in_pool() {
        let port = find_available_port(DEFAULT_PORT, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert!(port.is_ok());
        let p = port.unwrap();
        assert!((DEFAULT_PORT..=MAX_PORT).contains(&p));
    }
}
