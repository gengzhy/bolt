//! 证书指纹：BLAKE3(DER)，冒号分隔小写十六进制（实施方案 5.1）。

/// 计算证书 DER 的 BLAKE3 指纹（`aa:bb:cc:...`）。
pub fn cert_fingerprint(cert_der: &[u8]) -> String {
    let hash = blake3::hash(cert_der);
    hash.as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// 将指纹归一化（去空白、转小写），用于比较。
pub fn normalize(fp: &str) -> String {
    fp.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_shape() {
        let fp = cert_fingerprint(b"hello certificate");
        assert_eq!(fp.len(), 32 * 3 - 1);
        assert_eq!(fp.matches(':').count(), 31);
        assert_eq!(fp, cert_fingerprint(b"hello certificate"));
        assert_ne!(fp, cert_fingerprint(b"other"));
    }

    #[test]
    fn normalize_works() {
        assert_eq!(normalize(" AA:BB "), "aa:bb");
    }
}
