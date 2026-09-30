//! IP / 端口工具：端口池探测（8899~8950）、本机地址枚举。

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, UdpSocket};

use crate::error::{BtError, BtResult};
use crate::{DEFAULT_PORT, MAX_PORT};

/// 在端口池内寻找第一个可同时绑定 TCP 与 UDP 的端口。
///
/// 返回实际端口；若端口池耗尽返回 `BtError::PortUnavailable`。
pub fn find_available_port(preferred: u16, bind_addr: IpAddr) -> BtResult<u16> {
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
    Err(BtError::PortUnavailable)
}

/// 判断 TCP 端口当前是否可绑定。
pub fn tcp_port_free(addr: SocketAddr) -> bool {
    TcpListener::bind(addr).is_ok()
}

/// 检查是否为虚拟网卡、VPN、代理 Fake-IP 保留段或蜂窝移动数据网络接口。
pub fn is_virtual_or_cellular_adapter(name: &str, ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    // 1. 过滤 RFC 2544 基准测试保留段（198.18.0.0/15，Clash/Sing-box/Mihomo 等 TUN 虚拟网卡）
    if octets[0] == 198 && (octets[1] == 18 || octets[1] == 19) {
        return true;
    }
    // 2. 过滤回环与链路本地地址
    if ip.is_loopback() || ip.is_link_local() {
        return true;
    }
    // 3. 过滤常见虚拟网卡、VPN 及蜂窝移动网络接口名称（忽略大小写）
    let n = name.to_ascii_lowercase();
    if n.contains("vmware")
        || n.contains("vmnet")
        || n.contains("virtualbox")
        || n.contains("vbox")
        || n.contains("hyper-v")
        || n.contains("vethernet")
        || n.contains("wsl")
        || n.contains("wintun")
        || n.contains("tap")
        || n.contains("tun")
        || n.contains("mihomo")
        || n.contains("clash")
        || n.contains("tailscale")
        || n.contains("zerotier")
        || n.contains("rmnet")
        || n.contains("ccmni")
        || n.contains("pdp")
    {
        return true;
    }
    false
}

/// 枚举本机真实物理局域网 IPv4 地址及子网掩码（已过滤虚拟/代理/蜂窝接口）。
///
/// 返回 `(地址, 掩码)` 列表；网卡枚举失败时退化为组播路由探测单地址。
pub fn local_ipv4_nets() -> Vec<(Ipv4Addr, Ipv4Addr)> {
    let mut out = Vec::new();
    if let Ok(ifs) = if_addrs::get_if_addrs() {
        for i in ifs {
            if let if_addrs::IfAddr::V4(v4) = i.addr {
                if !is_virtual_or_cellular_adapter(&i.name, v4.ip) {
                    out.push((v4.ip, v4.netmask));
                }
            }
        }
    }
    if out.is_empty() {
        if let Some(ip) = probe_egress_ipv4() {
            if !is_virtual_or_cellular_adapter("", ip) {
                out.push((ip, Ipv4Addr::new(255, 255, 255, 0)));
            }
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

/// 计算 IPv4 子网定向广播地址（例如 10.192.58.152 / 255.255.255.0 -> 10.192.58.255）。
pub fn broadcast_addr(ip: Ipv4Addr, mask: Ipv4Addr) -> Ipv4Addr {
    let ip_u32 = u32::from(ip);
    let mask_u32 = u32::from(mask);
    Ipv4Addr::from(ip_u32 | (!mask_u32))
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
pub fn parse_target(target: &str, default_port: u16) -> BtResult<SocketAddr> {
    if let Ok(addr) = target.parse::<SocketAddr>() {
        return Ok(addr);
    }
    if let Ok(ip) = target.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, default_port));
    }
    Err(BtError::InvalidArgument)
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
    fn calculates_broadcast_addr() {
        let ip = Ipv4Addr::new(10, 192, 58, 152);
        let mask = Ipv4Addr::new(255, 255, 255, 0);
        assert_eq!(broadcast_addr(ip, mask), Ipv4Addr::new(10, 192, 58, 255));

        let ip2 = Ipv4Addr::new(192, 168, 1, 100);
        let mask2 = Ipv4Addr::new(255, 255, 0, 0);
        assert_eq!(broadcast_addr(ip2, mask2), Ipv4Addr::new(192, 168, 255, 255));
    }

    #[test]
    fn filters_virtual_adapters() {
        // 198.18.x.x 代理 Fake-IP
        assert!(is_virtual_or_cellular_adapter("Ethernet", Ipv4Addr::new(198, 18, 0, 1)));
        assert!(is_virtual_or_cellular_adapter("WLAN", Ipv4Addr::new(198, 19, 254, 1)));
        // 虚拟网卡名称
        assert!(is_virtual_or_cellular_adapter("VMware Network Adapter VMnet1", Ipv4Addr::new(192, 168, 88, 1)));
        assert!(is_virtual_or_cellular_adapter("Mihomo", Ipv4Addr::new(198, 18, 0, 1)));
        assert!(is_virtual_or_cellular_adapter("rmnet_data0", Ipv4Addr::new(10, 64, 1, 2)));
        // 真实网卡
        assert!(!is_virtual_or_cellular_adapter("WLAN", Ipv4Addr::new(10, 132, 77, 152)));
        assert!(!is_virtual_or_cellular_adapter("以太网", Ipv4Addr::new(192, 168, 3, 249)));
        assert!(!is_virtual_or_cellular_adapter("wlan0", Ipv4Addr::new(192, 168, 1, 105)));
    }

    #[test]
    fn finds_port_in_pool() {
        let port = find_available_port(DEFAULT_PORT, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert!(port.is_ok());
        let p = port.unwrap();
        assert!((DEFAULT_PORT..=MAX_PORT).contains(&p));
    }
}
