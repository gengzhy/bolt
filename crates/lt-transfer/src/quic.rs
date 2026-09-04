//! QUIC 传输（quinn，实施方案 7.1）。
//!
//! 多流并发：控制面 1 条流 + 每个文件独立双向流。

use std::net::SocketAddr;
use std::sync::Arc;

use lt_crypto::{tls, DeviceIdentity};
use lt_utils::{LtError, LtResult};
use socket2::{Domain, Protocol, Socket, Type};

use crate::conn::Pipe;

/// QUIC 跑在 UDP 上，系统默认 UDP 收发缓冲偏小（几十~几百 KB），
/// 高吞吐下来不及排空时内核会静默丢包、触发重传拖慢吞吐。
/// 交给 quinn 前强制设为 8MB（设置失败不致命，回落系统默认）。
const UDP_BUF_SIZE: usize = 8 * 1024 * 1024;

/// 创建带大收发缓冲的 UDP socket 并绑定 `addr`。
///
/// 复刻 `quinn::Endpoint::client/server` 内部的建套接字逻辑，但在
/// `bind` 前把 SO_RCVBUF/SO_SNDBUF 提到 [`UDP_BUF_SIZE`]。
fn udp_socket(addr: SocketAddr) -> LtResult<std::net::UdpSocket> {
    let socket =
        Socket::new(Domain::for_address(addr), Type::DGRAM, Some(Protocol::UDP)).map_err(|e| {
            tracing::error!(error = %e, %addr, "udp socket create failed");
            LtError::PortUnavailable
        })?;
    if addr.is_ipv6() {
        // 与 quinn 一致：IPv6 尽量双栈；失败仅降级不影响功能
        let _ = socket.set_only_v6(false);
    }
    // 尽力而为：个别平台/权限受限会失败，回落系统默认即可
    let _ = socket.set_recv_buffer_size(UDP_BUF_SIZE);
    let _ = socket.set_send_buffer_size(UDP_BUF_SIZE);
    socket.bind(&addr.into()).map_err(|e| {
        tracing::error!(error = %e, %addr, "udp bind failed");
        LtError::PortUnavailable
    })?;
    Ok(socket.into())
}

/// 公共传输参数调优（服务端/客户端必须一致，否则参数协商不对称）。
///
/// 局域网极致性能与稳定性优化（RFC 9000 & Quinn 0.11 生产标准）：
/// - 连接级流控解禁：receive_window 设为 VarInt::MAX，避免整连接总传输量（27MB~32MB）触顶诱发 FLOW_CONTROL_ERROR 强制断连。
/// - 流级窗口扩容：stream_receive_window 提至 32MB（5x 千兆 BDP），跑满 1Gbps 吞吐并杜绝流水线饥饿。
/// - 发送缓冲扩容：send_window 提至 64MB，提供充裕的未确认缓冲与快速重传空间。
/// - MTU 安全探测：RFC 9000 标准 initial_mtu 设为 1200，启用 DPLPMTUD（RFC 8899）自动向上探测至 1472/1500，杜绝移动 Wi-Fi 黑洞丢包。
/// - 并发与心跳：双向/单向流上限 64，保活 10s，空闲超时 90s，CUBIC 拥塞控制。
fn tune_transport(transport: &mut quinn::TransportConfig) {
    transport.keep_alive_interval(Some(std::time::Duration::from_secs(10)));
    transport.max_idle_timeout(Some(std::time::Duration::from_secs(90).try_into().unwrap()));
    transport.max_concurrent_bidi_streams(64u32.into());
    transport.max_concurrent_uni_streams(64u32.into());
    transport.stream_receive_window((32 * 1024 * 1024u32).into());
    transport.receive_window(quinn::VarInt::MAX);
    transport.send_window(64 * 1024 * 1024);
    transport.initial_mtu(1200);
    transport.mtu_discovery_config(Some(quinn::MtuDiscoveryConfig::default()));
    transport.congestion_controller_factory(Arc::new(quinn::congestion::CubicConfig::default()));
}

/// 构造 QUIC 服务端 Endpoint（绑定 addr，UDP 缓冲已加大）。
pub fn server_endpoint(addr: SocketAddr, identity: &DeviceIdentity) -> LtResult<quinn::Endpoint> {
    let rustls_cfg = tls::server_config(identity)?;
    let quic_cfg = quinn::crypto::rustls::QuicServerConfig::try_from(rustls_cfg)
        .map_err(|_| LtError::Internal)?;
    let mut cfg = quinn::ServerConfig::with_crypto(Arc::new(quic_cfg));
    let mut transport = quinn::TransportConfig::default();
    tune_transport(&mut transport);
    cfg.transport_config(Arc::new(transport));

    let socket = udp_socket(addr)?;
    let runtime = quinn::default_runtime().ok_or(LtError::Internal)?;
    quinn::Endpoint::new(quinn::EndpointConfig::default(), Some(cfg), socket, runtime).map_err(
        |e| {
            tracing::error!(error = %e, %addr, "quic bind failed");
            LtError::PortUnavailable
        },
    )
}

/// 构造 QUIC 客户端 Endpoint（本地任意端口，UDP 缓冲已加大）。
fn client_endpoint(identity: &DeviceIdentity) -> LtResult<(quinn::Endpoint, quinn::ClientConfig)> {
    let rustls_cfg = tls::quic_client_config(identity)?;
    let quic_cfg = quinn::crypto::rustls::QuicClientConfig::try_from(rustls_cfg)
        .map_err(|_| LtError::Internal)?;
    let mut cfg = quinn::ClientConfig::new(Arc::new(quic_cfg));
    let mut transport = quinn::TransportConfig::default();
    tune_transport(&mut transport);
    cfg.transport_config(Arc::new(transport));

    let bind: SocketAddr = "0.0.0.0:0".parse().unwrap();
    let socket = udp_socket(bind)?;
    let runtime = quinn::default_runtime().ok_or(LtError::Internal)?;
    let endpoint = quinn::Endpoint::new(quinn::EndpointConfig::default(), None, socket, runtime)
        .map_err(|_| LtError::PortUnavailable)?;
    Ok((endpoint, cfg))
}

/// 拨号 QUIC 连接并打开控制流。
pub async fn dial(
    target: SocketAddr,
    identity: &DeviceIdentity,
) -> LtResult<(Pipe, quinn::Connection, quinn::Endpoint)> {
    let (mut endpoint, cfg) = client_endpoint(identity)?;
    endpoint.set_default_client_config(cfg);
    let connecting = endpoint
        .connect(target, "localtransfer")
        .map_err(|_| LtError::ConnectTimeout)?;
    let connection = connecting.await.map_err(|e| {
        tracing::debug!(error = %e, "quic connect failed");
        LtError::ConnectTimeout
    })?;
    let (send, recv) = connection
        .open_bi()
        .await
        .map_err(|_| LtError::ConnectTimeout)?;
    let pipe = Pipe {
        reader: Box::new(recv),
        writer: Box::new(send),
    };
    Ok((pipe, connection, endpoint))
}

/// 从入站连接打开控制流（服务端侧）。
pub fn accept_pipe(connection: &quinn::Connection) -> Pipe {
    // 由 accept_bi 得到的流在 session 层建立；此处仅提供类型辅助
    let _ = connection;
    unreachable!("use accept_bi directly")
}

/// 将 quinn 双向流转为 Pipe。
pub fn pipe_from_bi(send: quinn::SendStream, recv: quinn::RecvStream) -> Pipe {
    Pipe {
        reader: Box::new(recv),
        writer: Box::new(send),
    }
}
