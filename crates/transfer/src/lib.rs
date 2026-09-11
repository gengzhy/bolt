//! transfer：传输引擎（实施方案第四、七节）。
//!
//! - [`protocol`] 协议 V2 编解码（含畸形包处理，未知指令回 -13 并保持连接）；
//! - [`conn`] 帧式读写管道（QUIC stream / TCP 统一抽象）；
//! - [`quic`] QUIC 传输（quinn）；
//! - [`tcp`] TCP + TLS 1.3 降级传输；
//! - [`session`] 会话状态机（HELLO / 配对 / 心跳 / 双向请求分发）；
//! - [`send`] / [`recv`] 发送与接收流水线（分片、累积确认、断点续传、哈希校验）；
//! - [`engine`] 监听器与拨号入口（双协议自适应：QUIC 失败降级 TCP）。

pub mod conn;
pub mod engine;
pub mod protocol;
pub mod quic;
pub mod recv;
pub mod send;
pub mod session;
pub mod tcp;

pub use engine::{EngineConfig, TransferEngine};
pub use protocol::Message;
pub use session::{Session, SessionInfo, TransportKind};
