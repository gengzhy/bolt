//! TLS 1.3 配置（rustls + ring，实施方案 5.4）。
//!
//! V1 全链路强制 TLS 1.3，不提供明文模式。
//! 身份绑定策略：证书用于信道加密；设备身份指纹经 HELLO 报文交换，
//! 与配对验证码共同完成防中间人校验（TOFU 由 [`crate::trust`] 承担）。

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, ServerConfig, SignatureScheme};

use utils::{LtError, LtResult};

use crate::identity::DeviceIdentity;
use crate::ALPN;

/// 服务端配置：仅 TLS 1.3 + 自签证书。
pub fn server_config(identity: &DeviceIdentity) -> LtResult<ServerConfig> {
    crate::ensure_provider();
    let mut cfg = ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_no_client_auth()
        .with_single_cert(identity.cert_chain(), identity.private_key())
        .map_err(|e| {
            tracing::error!(error = %e, "tls server config failed");
            LtError::Internal
        })?;
    // QUIC 要求服务端也声明 ALPN，否则握手报 "peer doesn't support any known protocol"
    cfg.alpn_protocols = vec![ALPN.to_vec()];
    Ok(cfg)
}

/// 客户端配置：接受任意自签证书（指纹在应用层校验），并出示自身证书。
pub fn client_config(identity: &DeviceIdentity) -> LtResult<ClientConfig> {
    crate::ensure_provider();
    let mut cfg = ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyServerCert))
        .with_client_auth_cert(identity.cert_chain(), identity.private_key())
        .map_err(|e| {
            tracing::error!(error = %e, "tls client config failed");
            LtError::Internal
        })?;
    cfg.alpn_protocols = vec![ALPN.to_vec()];
    cfg.enable_early_data = false;
    Ok(cfg)
}

/// QUIC 客户端专用（附加 ALPN；quinn 要求非空）。
pub fn quic_client_config(identity: &DeviceIdentity) -> LtResult<ClientConfig> {
    client_config(identity)
}

/// 接受任意服务端证书的校验器（自签生态）。
///
/// 安全说明：信道加密仍由 TLS 1.3 保证；对端身份的真实性由
/// HELLO 指纹交换 + 6 位配对验证码人工比对 + TOFU 信任库三层保障。
#[derive(Debug)]
struct AcceptAnyServerCert;

impl ServerCertVerifier for AcceptAnyServerCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ED25519,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_configs() {
        let tmp = std::env::temp_dir().join(format!("lt-tls-{}", std::process::id()));
        let id = DeviceIdentity::load_or_create(&tmp, "tls-test").unwrap();
        assert!(server_config(&id).is_ok());
        assert!(client_config(&id).is_ok());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
