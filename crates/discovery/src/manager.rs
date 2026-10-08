//! 发现管理器：统一编排三通道（实施方案第六节）。
//!
//! - `start` 启动 mDNS 广播/浏览 + UDP 探测监听/广播 + 过期清扫线程；
//! - 隐身模式：不广播、不响应探测（仅被动浏览），实施方案 9.4；
//! - `stop` 幂等停止全部线程。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::device::Device;
use crate::device_list::DeviceList;
use crate::mdns::{AnnounceInfo, MdnsChannel};
use crate::udp_probe::UdpProbe;

/// 发现配置。
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// 本机信息（uuid/名称/类型/端口），用于广播。
    pub announce: AnnounceInfo,
    /// 隐身模式：不广播、不响应探测。
    pub stealth: bool,
    /// 启用 mDNS 通道（Android 上由 NSD 桥替代，置 false）。
    pub use_mdns: bool,
    /// 启用 UDP 探测通道。
    pub use_udp_probe: bool,
    /// 设备过期时间（默认 10s）。
    pub ttl_secs: u64,
    /// UDP 探测广播间隔（默认 3s）。
    pub broadcast_secs: u64,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        DiscoveryConfig {
            announce: AnnounceInfo {
                uuid: String::new(),
                name: String::new(),
                device_type: 1,
                quic_port: 8899,
                tcp_port: 8899,
                proto_ver: 2,
                stealth: false,
                prefer_tcp: false,
            },
            stealth: false,
            use_mdns: true,
            use_udp_probe: true,
            ttl_secs: 25,
            broadcast_secs: 3,
        }
    }
}

struct Running {
    stop: Arc<AtomicBool>,
    handles: Vec<JoinHandle<()>>,
    /// 探测通道句柄（供 [`DiscoveryManager::probe_now`] 即时广播）。
    probe: Option<Arc<UdpProbe>>,
    /// 本机 UUID（探测广播携带，供对端过滤）。
    own_uuid: String,
    announce: AnnounceInfo,
    mdns: Option<Arc<Mutex<Option<MdnsChannel>>>>,
    last_nets: Arc<Mutex<Vec<(std::net::Ipv4Addr, std::net::Ipv4Addr)>>>,
}

/// 发现管理器。
pub struct DiscoveryManager {
    devices: Arc<DeviceList>,
    running: Mutex<Option<Running>>,
    transfer_active: Arc<AtomicBool>,
}

impl DiscoveryManager {
    pub fn new(devices: Arc<DeviceList>) -> DiscoveryManager {
        DiscoveryManager {
            devices,
            running: Mutex::new(None),
            transfer_active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 设置传输活跃状态：传输期间雷达全面静默，100% 物理信道让给数据流与 ACK
    pub fn set_transfer_active(&self, active: bool) {
        self.transfer_active.store(active, Ordering::Relaxed);
    }

    pub fn is_transfer_active(&self) -> bool {
        self.transfer_active.load(Ordering::Relaxed)
    }

    pub fn devices(&self) -> Arc<DeviceList> {
        self.devices.clone()
    }

    pub fn is_running(&self) -> bool {
        self.running.lock().unwrap().is_some()
    }

    /// 启动发现。重复调用先停旧再启新（配置热更场景）。
    pub fn start(&self, cfg: DiscoveryConfig) -> std::io::Result<()> {
        self.stop();
        let stop = Arc::new(AtomicBool::new(false));
        let mut handles = Vec::new();

        let own_uuid = cfg.announce.uuid.clone();
        // 本机全部非环回 IPv4（含掩码）：mDNS 需要逐网卡注册地址；
        // 探测应答按对端子网挑选。枚举失败兜底环回（仅本机自发现降级）。
        let local_nets = utils::net::local_ipv4_nets();
        let local_ips: Vec<std::net::Ipv4Addr> = if local_nets.is_empty() {
            tracing::warn!("未枚举到本机 IPv4 地址，发现广播降级为仅环回可见");
            vec![std::net::Ipv4Addr::new(127, 0, 0, 1)]
        } else {
            local_nets.iter().map(|(ip, _)| *ip).collect()
        };
        let last_nets = Arc::new(Mutex::new(local_nets.clone()));

        // ---- mDNS 通道 ----
        let mut mdns_handle: Option<Arc<Mutex<Option<MdnsChannel>>>> = None;
        if cfg.use_mdns {
            let mut channel = MdnsChannel::new()?;
            if !cfg.stealth {
                if let Err(e) = channel.announce(&cfg.announce, &local_ips) {
                    tracing::warn!(err = %e, "mDNS 广播启动失败（继续浏览）");
                }
            }
            match channel.browse() {
                Ok(receiver) => {
                    let devices = self.devices.clone();
                    let stop_c = stop.clone();
                    let channel_cell = Arc::new(Mutex::new(Some(channel)));
                    mdns_handle = Some(channel_cell.clone());
                    handles.push(thread::Builder::new().name("bt-mdns-browse".into()).spawn(
                        move || {
                            crate::mdns::browse_loop(
                                receiver,
                                stop_c,
                                move |dev| {
                                    devices.upsert(dev);
                                },
                                move |uuid| {
                                    tracing::debug!(%uuid, "mdns service removed");
                                },
                            );
                            if let Ok(mut g) = channel_cell.lock() {
                                if let Some(ch) = g.take() {
                                    ch.shutdown();
                                }
                            }
                        },
                    )?);
                }
                Err(e) => tracing::warn!(err = %e, "mDNS 浏览启动失败"),
            }
        }

        // ---- UDP 探测通道 ----
        let mut probe_handle: Option<Arc<UdpProbe>> = None;
        if cfg.use_udp_probe {
            let probe = Arc::new(UdpProbe::new(&local_nets)?);
            probe_handle = Some(probe.clone());
            let stealth = cfg.stealth;
            let announce = cfg.announce.clone();
            let devices_in = self.devices.clone();
            let stop_c = stop.clone();
            let probe_c = probe.clone();
            let listen_uuid = own_uuid.clone();
            handles.push(
                thread::Builder::new()
                    .name("bt-udp-probe".into())
                    .spawn(move || {
                        probe_c.listen_loop(
                            listen_uuid,
                            stop_c,
                            move |from| {
                                if stealth {
                                    return None; // 隐身：不响应探测
                                }
                                // 多网卡主机按对端子网挑选应答 IP（动态获取最新网卡）
                                let current_nets = utils::net::local_ipv4_nets();
                                let reply_ip = utils::net::pick_ip_for_peer(&current_nets, from.ip())
                                    .or_else(|| current_nets.first().map(|(ip, _)| *ip))
                                    .unwrap_or(std::net::Ipv4Addr::new(127, 0, 0, 1));
                                Some(Device {
                                    uuid: announce.uuid.clone(),
                                    name: announce.name.clone(),
                                    device_type: announce.device_type,
                                    ip: reply_ip.to_string(),
                                    quic_port: announce.quic_port,
                                    tcp_port: announce.tcp_port,
                                    proto_ver: announce.proto_ver,
                                    stealth,
                                    prefer_tcp: announce.prefer_tcp,
                                    source: "udp_probe".into(),
                                    last_seen_unix: Device::now_unix(),
                                    })
                            },
                            move |dev| {
                                devices_in.upsert(dev);
                            },
                        );
                    })?,
            );

            // 主动广播线程（隐身模式不广播）
            if !cfg.stealth {
                let interval = Duration::from_secs(cfg.broadcast_secs.max(1));
                let stop_c = stop.clone();
                let probe_c = probe.clone();
                let uuid = own_uuid.clone();
                let ann = cfg.announce.clone();
                let mdns_c = mdns_handle.clone();
                let nets_tracker = last_nets.clone();
                let devices_in_bcast = self.devices.clone();
                let is_transfer_active = self.transfer_active.clone();
                handles.push(thread::Builder::new().name("bt-udp-bcast".into()).spawn(
                    move || {
                        while !stop_c.load(Ordering::Relaxed) {
                            if is_transfer_active.load(Ordering::Relaxed) {
                                // 处于高速传输状态：雷达完全静默，0广播、0网卡枚举，100% 物理信道让给数据流与 ACK
                                let mut waited = Duration::ZERO;
                                while waited < Duration::from_secs(1)
                                    && !stop_c.load(Ordering::Relaxed)
                                    && is_transfer_active.load(Ordering::Relaxed)
                                {
                                    thread::sleep(Duration::from_millis(200));
                                    waited += Duration::from_millis(200);
                                }
                                continue;
                            }

                            // 1. 动态网卡接口感知与自愈
                            let current_nets = utils::net::local_ipv4_nets();
                            probe_c.update_interfaces(&current_nets);

                            // 若网卡有增删变动，重新 announce mDNS（桌面端）并通知前端更新本地 IP
                            let mut tracker = nets_tracker.lock().unwrap();
                            if *tracker != current_nets {
                                *tracker = current_nets.clone();
                                if let Some(ref mdns_mutex) = mdns_c {
                                    if let Ok(mut guard) = mdns_mutex.lock() {
                                        if let Some(ch) = guard.as_mut() {
                                            let ips: Vec<_> = current_nets.iter().map(|(ip, _)| *ip).collect();
                                            ch.stop_announce();
                                            let _ = ch.announce(&ann, &ips);
                                        }
                                    }
                                }
                                devices_in_bcast.notify();
                            }
                            drop(tracker);

                            // 2. 构造当前最新本地 Device 信息并发送广播
                            let best_ip = current_nets
                                .first()
                                .map(|(ip, _)| *ip)
                                .unwrap_or(std::net::Ipv4Addr::new(127, 0, 0, 1));
                            let me_dev = Device {
                                uuid: uuid.clone(),
                                name: ann.name.clone(),
                                device_type: ann.device_type,
                                ip: best_ip.to_string(),
                                quic_port: ann.quic_port,
                                tcp_port: ann.tcp_port,
                                proto_ver: ann.proto_ver,
                                stealth: false,
                                prefer_tcp: ann.prefer_tcp,
                                source: "udp_probe".into(),
                                last_seen_unix: Device::now_unix(),
                            };

                            let _ = probe_c.send_probe(&uuid, Some(&me_dev));

                            // 分片睡眠以便及时响应停止
                            let mut waited = Duration::ZERO;
                            while waited < interval && !stop_c.load(Ordering::Relaxed) {
                                thread::sleep(Duration::from_millis(200));
                                waited += Duration::from_millis(200);
                            }
                        }
                    },
                )?);
            }
        }

        // ---- 过期清扫线程 ----
        {
            let devices = self.devices.clone();
            let stop_c = stop.clone();
            let ttl = cfg.ttl_secs;
            let is_transfer_active_sweep = self.transfer_active.clone();
            handles.push(
                thread::Builder::new()
                    .name("bt-dev-sweep".into())
                    .spawn(move || {
                        while !stop_c.load(Ordering::Relaxed) {
                            thread::sleep(Duration::from_secs(3));
                            if stop_c.load(Ordering::Relaxed) {
                                break;
                            }
                            // 传输期间跳过清理，防止正在传输的对端因心跳静默被误剔除
                            if !is_transfer_active_sweep.load(Ordering::Relaxed) {
                                devices.sweep_expired(ttl);
                            }
                        }
                    })?,
            );
        }

        *self.running.lock().unwrap() = Some(Running {
            stop,
            handles,
            probe: probe_handle,
            own_uuid,
            announce: cfg.announce,
            mdns: mdns_handle,
            last_nets,
        });
        Ok(())
    }

    /// 停止全部发现线程（幂等）。
    pub fn stop(&self) {
        let running = self.running.lock().unwrap().take();
        if let Some(r) = running {
            r.stop.store(true, Ordering::Relaxed);
            for h in r.handles {
                let _ = h.join();
            }
        }
    }

    /// 即时探测一次（不重启通道）：动态刷新网卡并立刻广播一轮 UDP 探测包。
    ///
    /// 供「刷新」按钮使用——绝不可用 stop+start 实现（join 线程可达数秒，
    /// 在 UI 线程调用会 ANR）。
    pub fn probe_now(&self) {
        let guard = self.running.lock().unwrap();
        if let Some(r) = guard.as_ref() {
            if let Some(probe) = &r.probe {
                let current_nets = utils::net::local_ipv4_nets();
                probe.update_interfaces(&current_nets);

                let mut tracker = r.last_nets.lock().unwrap();
                if *tracker != current_nets {
                    *tracker = current_nets.clone();
                    if let Some(ref mdns_mutex) = r.mdns {
                        if let Ok(mut g) = mdns_mutex.lock() {
                            if let Some(ch) = g.as_mut() {
                                let ips: Vec<_> = current_nets.iter().map(|(ip, _)| *ip).collect();
                                ch.stop_announce();
                                let _ = ch.announce(&r.announce, &ips);
                            }
                        }
                    }
                    self.devices.notify();
                }
                drop(tracker);

                let best_ip = current_nets
                    .first()
                    .map(|(ip, _)| *ip)
                    .unwrap_or(std::net::Ipv4Addr::new(127, 0, 0, 1));
                let me_dev = Device {
                    uuid: r.own_uuid.clone(),
                    name: r.announce.name.clone(),
                    device_type: r.announce.device_type,
                    ip: best_ip.to_string(),
                    quic_port: r.announce.quic_port,
                    tcp_port: r.announce.tcp_port,
                    proto_ver: r.announce.proto_ver,
                    stealth: r.announce.stealth,
                    prefer_tcp: r.announce.prefer_tcp,
                    source: "udp_probe".into(),
                    last_seen_unix: Device::now_unix(),
                };

                let _ = probe.send_probe(&r.own_uuid, Some(&me_dev));
            }
        }
    }

    /// 手动添加设备（直连场景，实施方案 6.4）：
    /// 未知 uuid 时用 `manual:<ip>` 占位，连接成功后由 HELLO 校正。
    pub fn add_manual(&self, ip: &str, port: u16) -> Device {
        let dev = Device {
            uuid: format!("manual:{ip}"),
            name: format!("{ip}:{port}"),
            device_type: 0,
            ip: ip.to_string(),
            quic_port: port,
            tcp_port: port,
            proto_ver: 2,
            stealth: false,
            prefer_tcp: false,
            source: "manual".into(),
            last_seen_unix: Device::now_unix(),
        };
        self.devices.upsert(dev.clone());
        dev
    }
}

impl Drop for DiscoveryManager {
    fn drop(&mut self) {
        self.stop();
    }
}
