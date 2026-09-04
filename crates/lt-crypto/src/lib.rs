//! lt-crypto：安全层。
//!
//! 模块划分：
//! - [`identity`] 首次启动生成 Ed25519 密钥对 + 自签证书（10 年），与设备 UUID 一同持久化；
//! - [`fingerprint`] BLAKE3 证书指纹（冒号分隔，设置页展示供人工比对）；
//! - [`pairing`] 由双端指纹派生的 6 位配对验证码；
//! - [`trust`] TOFU 信任库（指纹变更即 -14）；
//! - [`tls`] 全链路 TLS 1.3 配置（rustls + ring）。

pub mod fingerprint;
pub mod identity;
pub mod pairing;
pub mod tls;
pub mod trust;

pub use fingerprint::cert_fingerprint;
pub use identity::DeviceIdentity;
pub use pairing::verification_code;
pub use trust::{TrustStatus, TrustStore};

/// QUIC ALPN 标识。
pub const ALPN: &[u8] = b"lt/1";

/// 安装进程级 rustls CryptoProvider（ring）。重复调用安全。
pub fn ensure_provider() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}
