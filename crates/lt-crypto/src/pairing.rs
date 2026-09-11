//! 配对验证码（实施方案 5.2）。
//!
//! 由双端证书指纹（排序后拼接）经 BLAKE3 派生 4 位十进制码。
//! 双端独立计算结果一致；中间人伪造证书必然导致两侧验证码不一致。

/// 由双方指纹派生 4 位验证码（指纹交换顺序不影响结果）。
pub fn verification_code(fp_a: &str, fp_b: &str) -> String {
    let a = crate::fingerprint::normalize(fp_a);
    let b = crate::fingerprint::normalize(fp_b);
    let (x, y) = if a <= b { (a, b) } else { (b, a) };
    let mut input = Vec::with_capacity(x.len() + y.len() + 1);
    input.extend_from_slice(x.as_bytes());
    input.push(b'|');
    input.extend_from_slice(y.as_bytes());
    let hash = blake3::hash(&input);
    let bytes = hash.as_bytes();
    let n = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 10_000;
    format!("{n:04}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(seed: &[u8]) -> String {
        crate::fingerprint::cert_fingerprint(seed)
    }

    #[test]
    fn symmetric() {
        let a = fp(b"certA");
        let b = fp(b"certB");
        assert_eq!(verification_code(&a, &b), verification_code(&b, &a));
        assert_eq!(verification_code(&a, &b).len(), 4);
        assert!(verification_code(&a, &b)
            .chars()
            .all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn different_certs_different_codes() {
        let a = fp(b"certA");
        let b = fp(b"certB");
        let c = fp(b"certC");
        // 不同指纹对大概率不同（这里构造的输入必然不同）
        assert_ne!(verification_code(&a, &b), verification_code(&a, &c));
    }
}
