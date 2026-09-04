//! 设备身份：Ed25519 密钥对 + 自签证书 + 设备 UUID（实施方案 5.1）。
//!
//! 首次启动生成，持久化于应用私有目录 `<data_dir>/identity/`；
//! 重装应用视为新设备。

use std::fs;
use std::path::Path;
use std::sync::RwLock;

use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use lt_utils::{LtError, LtResult};

use crate::fingerprint::cert_fingerprint;

/// 证书有效期：10 年。
const VALIDITY_DAYS: i64 = 3650;

/// 本机设备身份（证书 + 私钥 + UUID）。
pub struct DeviceIdentity {
    /// 设备 UUID（与证书一同持久化）
    pub uuid: String,
    /// 设备显示名称（HELLO 携带；可运行时改名，内部可变）
    device_name: RwLock<String>,
    /// 证书 DER
    pub cert_der: Vec<u8>,
    /// 证书 PEM（持久化用）
    pub cert_pem: String,
    /// PKCS#8 密钥 DER
    key_der: Vec<u8>,
    /// 密钥 PEM（持久化用）
    key_pem: String,
    /// BLAKE3 证书指纹（冒号分隔）
    pub fingerprint: String,
}

impl DeviceIdentity {
    /// 加载或首次生成设备身份。
    ///
    /// 目录结构：`data_dir/identity/{cert.pem, key.pem, device.json}`。
    pub fn load_or_create(data_dir: &Path, device_name: &str) -> LtResult<DeviceIdentity> {
        let dir = data_dir.join("identity");
        let cert_path = dir.join("cert.pem");
        let key_path = dir.join("key.pem");
        let meta_path = dir.join("device.json");

        if cert_path.exists() && key_path.exists() && meta_path.exists() {
            return Self::load(&dir, &cert_path, &key_path, &meta_path);
        }

        let identity = Self::generate(device_name)?;
        fs::create_dir_all(&dir).map_err(|_| LtError::PermissionDenied)?;
        fs::write(&cert_path, &identity.cert_pem).map_err(|_| LtError::Internal)?;
        fs::write(&key_path, &identity.key_pem).map_err(|_| LtError::Internal)?;
        let meta = serde_json::json!({
            "uuid": identity.uuid,
            "device_name": identity.device_name(),
            "fingerprint": identity.fingerprint,
        });
        fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap())
            .map_err(|_| LtError::Internal)?;

        // 私有目录权限：尽力收紧（Windows 由应用目录本身隔离）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600));
        }
        Ok(identity)
    }

    fn load(
        dir: &Path,
        cert_path: &Path,
        key_path: &Path,
        meta_path: &Path,
    ) -> LtResult<DeviceIdentity> {
        let cert_pem = fs::read_to_string(cert_path).map_err(|_| LtError::Internal)?;
        let key_pem = fs::read_to_string(key_path).map_err(|_| LtError::Internal)?;
        let meta_raw = fs::read_to_string(meta_path).map_err(|_| LtError::Internal)?;
        let meta: serde_json::Value =
            serde_json::from_str(&meta_raw).map_err(|_| LtError::Internal)?;
        let uuid = meta["uuid"].as_str().ok_or(LtError::Internal)?.to_string();
        let device_name = meta["device_name"].as_str().unwrap_or("device").to_string();

        let cert_der = pem_to_der(&cert_pem, "CERTIFICATE")?;
        let key_der = pem_to_der(&key_pem, "PRIVATE KEY")?;
        let fingerprint = cert_fingerprint(&cert_der);
        let _ = dir;
        Ok(DeviceIdentity {
            uuid,
            device_name: RwLock::new(device_name),
            cert_der,
            cert_pem,
            key_der,
            key_pem,
            fingerprint,
        })
    }

    fn generate(device_name: &str) -> LtResult<DeviceIdentity> {
        crate::ensure_provider();

        // Ed25519 密钥对（rcgen 内部经 ring 生成，PKCS#8 封装）
        let key_pair = KeyPair::generate_for(&rcgen::PKCS_ED25519).map_err(|e| {
            tracing::error!(error = %e, "ed25519 keygen failed");
            LtError::Internal
        })?;

        // 自签证书参数（设备名仅写入 DN/CN；SAN 保持空，避免非 ASCII 校验问题）
        let mut params =
            CertificateParams::new(Vec::<String>::new()).map_err(|_| LtError::Internal)?;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, device_name);
        dn.push(DnType::OrganizationName, "LocalTransfer");
        params.distinguished_name = dn;
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now;
        params.not_after = now + time::Duration::days(VALIDITY_DAYS);

        let cert = params.self_signed(&key_pair).map_err(|e| {
            tracing::error!(error = %e, "self-sign failed");
            LtError::Internal
        })?;

        let cert_der = cert.der().to_vec();
        let cert_pem = cert.pem();
        let key_der = key_pair.serialize_der();
        let key_pem = key_pair.serialize_pem();
        let fingerprint = cert_fingerprint(&cert_der);

        Ok(DeviceIdentity {
            uuid: lt_utils::id::new_uuid(),
            device_name: RwLock::new(device_name.to_string()),
            cert_der,
            cert_pem,
            key_der,
            key_pem,
            fingerprint,
        })
    }

    /// 当前设备显示名称（HELLO 携带；引擎共享同一 Arc，改名即时生效）。
    pub fn device_name(&self) -> String {
        self.device_name.read().unwrap().clone()
    }

    /// 更新显示名称并持久化 device.json。
    ///
    /// uuid / 指纹不变；证书 CN 仅首签时写入（装饰性），改名不重签——
    /// 重签会改变指纹，触发对端 TOFU「指纹变更」硬拒绝。
    pub fn update_device_name(&self, data_dir: &Path, name: &str) -> LtResult<()> {
        *self.device_name.write().unwrap() = name.to_string();
        let meta = serde_json::json!({
            "uuid": self.uuid,
            "device_name": name,
            "fingerprint": self.fingerprint,
        });
        let meta_path = data_dir.join("identity").join("device.json");
        fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap())
            .map_err(|_| LtError::Internal)?;
        Ok(())
    }

    /// rustls 证书链（单证书）。
    pub fn cert_chain(&self) -> Vec<CertificateDer<'static>> {
        vec![CertificateDer::from(self.cert_der.clone())]
    }

    /// rustls PKCS#8 私钥。
    pub fn private_key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key_der.clone()))
    }
}

/// 从 PEM 文本中提取 DER（取 BEGIN/END 之间的 base64 主体）。
fn pem_to_der(pem: &str, label: &str) -> LtResult<Vec<u8>> {
    let body: String = pem
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with("-----") && !t.is_empty()
        })
        .collect::<Vec<_>>()
        .join("");
    let _ = label; // label 仅作文档说明
    decode_b64(&body).ok_or(LtError::Internal)
}

fn decode_b64(s: &str) -> Option<Vec<u8>> {
    // 极简 base64 解码，避免新增依赖
    let table: Vec<u8> =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".to_vec();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits = 0;
    for c in s.chars() {
        let v = if c == '=' {
            break;
        } else {
            let pos = table.iter().position(|&x| x as char == c)?;
            pos as u32
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_reload() {
        let tmp = std::env::temp_dir().join(format!("lt-id-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let id1 = DeviceIdentity::load_or_create(&tmp, "测试设备").unwrap();
        assert!(!id1.uuid.is_empty());
        assert!(!id1.fingerprint.is_empty());
        assert!(id1.fingerprint.contains(':'));

        let id2 = DeviceIdentity::load_or_create(&tmp, "测试设备").unwrap();
        assert_eq!(id1.uuid, id2.uuid, "identity must persist");
        assert_eq!(id1.fingerprint, id2.fingerprint);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn rustls_materials_valid() {
        let tmp = std::env::temp_dir().join(format!("lt-id-test2-{}", std::process::id()));
        let id = DeviceIdentity::load_or_create(&tmp, "tls-test").unwrap();
        assert_eq!(id.cert_chain().len(), 1);
        let _ = id.private_key();
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn rename_persists_without_fingerprint_change() {
        let tmp = std::env::temp_dir().join(format!("lt-id-test3-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let id1 = DeviceIdentity::load_or_create(&tmp, "旧随机名").unwrap();
        let (uuid, fp) = (id1.uuid.clone(), id1.fingerprint.clone());

        id1.update_device_name(&tmp, "新设备名").unwrap();
        assert_eq!(id1.device_name(), "新设备名");

        let id2 = DeviceIdentity::load_or_create(&tmp, "新设备名").unwrap();
        assert_eq!(id2.device_name(), "新设备名");
        assert_eq!(id2.uuid, uuid, "改名不得改变 uuid");
        assert_eq!(
            id2.fingerprint, fp,
            "改名不得改变指纹（否则触发 TOFU 拒绝）"
        );
        let _ = fs::remove_dir_all(&tmp);
    }
}
