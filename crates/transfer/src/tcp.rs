//! TCP + TLS 1.3 降级传输（实施方案 7.1）。
//!
//! QUIC 握手失败/UDP 被防火墙拦截时自动降级；
//! V1 单连接承载控制面与数据面（4 路并发连接为后续吞吐优化项）。

use std::net::SocketAddr;
use std::sync::Arc;

use crypto::{tls, DeviceIdentity};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use utils::{BtError, BtResult};

use crate::conn::Pipe;

/// 绑定 TCP 监听器。
pub async fn listen(addr: SocketAddr) -> BtResult<TcpListener> {
    TcpListener::bind(addr).await.map_err(|e| {
        tracing::error!(error = %e, %addr, "tcp bind failed");
        BtError::PortUnavailable
    })
}

/// TLS acceptor（服务端）。
pub fn acceptor(identity: &DeviceIdentity) -> BtResult<TlsAcceptor> {
    let cfg = tls::server_config(identity)?;
    Ok(TlsAcceptor::from(Arc::new(cfg)))
}

/// TLS connector（客户端）。
pub fn connector(identity: &DeviceIdentity) -> BtResult<TlsConnector> {
    let cfg = tls::client_config(identity)?;
    Ok(TlsConnector::from(Arc::new(cfg)))
}

/// 复制底层 socket 句柄得到「kill 句柄」（std 流：同步 shutdown 可用）：
/// tokio TcpStream 非 Clone，经 BorrowedSocket::try_clone_to_owned 拿同
/// socket 的独立句柄。
fn kill_handle(stream: &TcpStream) -> BtResult<std::net::TcpStream> {
    #[cfg(unix)]
    let dup = {
        use std::os::unix::io::AsFd;
        let owned = stream
            .as_fd()
            .try_clone_to_owned()
            .map_err(|_| BtError::Internal)?;
        std::net::TcpStream::from(owned)
    };
    #[cfg(windows)]
    let dup = {
        use std::os::windows::io::AsSocket;
        let owned = stream
            .as_socket()
            .try_clone_to_owned()
            .map_err(|_| BtError::Internal)?;
        std::net::TcpStream::from(owned)
    };
    Ok(dup)
}

/// 服务端接受一个连接并完成 TLS 握手。
///
/// 额外返回原始 TCP 流的克隆（「kill 句柄」）：关闭会话时 shutdown 它，
/// 让对端读循环立即 EOF 感知断开（不等 90s 心跳）。
pub async fn accept_one(
    listener: &TcpListener,
    acceptor: &TlsAcceptor,
) -> BtResult<(Pipe, SocketAddr, std::net::TcpStream)> {
    let (stream, addr) = listener.accept().await.map_err(BtError::from)?;
    let _ = stream.set_nodelay(true);
    bump_buffers(&stream);
    let kill = kill_handle(&stream)?;
    let tls = acceptor.accept(stream).await.map_err(|e| {
        tracing::debug!(error = %e, %addr, "tls accept failed");
        BtError::ConnectTimeout
    })?;
    let (reader, writer) = tokio::io::split(tls);
    Ok((
        Pipe {
            reader: Box::new(reader),
            writer: Box::new(writer),
        },
        addr,
        kill,
    ))
}

/// 客户端拨号（TCP + TLS）。返回管道与 kill 句柄（见 [`accept_one`]）。
pub async fn dial(
    target: SocketAddr,
    identity: &DeviceIdentity,
) -> BtResult<(Pipe, std::net::TcpStream)> {
    let stream = TcpStream::connect(target)
        .await
        .map_err(|_| BtError::ConnectTimeout)?;
    let _ = stream.set_nodelay(true);
    bump_buffers(&stream);
    let kill = kill_handle(&stream)?;
    let connector = connector(identity)?;
    let server_name =
        rustls::pki_types::ServerName::try_from("bolt.local").map_err(|_| BtError::Internal)?;
    let tls = connector.connect(server_name, stream).await.map_err(|e| {
        tracing::debug!(error = %e, %target, "tls connect failed");
        BtError::ConnectTimeout
    })?;
    let (reader, writer) = tokio::io::split(tls);
    Ok((
        Pipe {
            reader: Box::new(reader),
            writer: Box::new(writer),
        },
        kill,
    ))
}

/// 收发缓冲调优（按带宽时延积预留，实施方案 7.1-3）。
fn bump_buffers(stream: &TcpStream) {
    const BUF: u32 = 4 * 1024 * 1024;
    socket2_set(stream, BUF);
}

#[cfg(unix)]
fn socket2_set(stream: &TcpStream, buf: u32) {
    let sock = socket2::SockRef::from(stream);
    let _ = sock.set_recv_buffer_size(buf as usize);
    let _ = sock.set_send_buffer_size(buf as usize);
}

#[cfg(windows)]
fn socket2_set(stream: &TcpStream, buf: u32) {
    use std::os::windows::io::AsRawSocket;
    #[link(name = "ws2_32")]
    extern "system" {
        fn setsockopt(s: usize, level: i32, optname: i32, optval: *const u8, optlen: i32) -> i32;
    }
    const SOL_SOCKET: i32 = 0xffff;
    const SO_RCVBUF: i32 = 0x1002;
    const SO_SNDBUF: i32 = 0x1001;
    let sock = stream.as_raw_socket() as usize;
    let v = buf.to_ne_bytes();
    unsafe {
        setsockopt(sock, SOL_SOCKET, SO_RCVBUF, v.as_ptr(), 4);
        setsockopt(sock, SOL_SOCKET, SO_SNDBUF, v.as_ptr(), 4);
    }
}
