//! 设备 UUID 生成。

use rand::RngExt;

/// 生成 RFC4122 v4 风格的 UUID（不引入 uuid crate，保持零额外依赖）。
pub fn new_uuid() -> String {
    let mut rng = rand::rng();
    let mut b = [0u8; 16];
    rng.fill(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12],
        b[13], b[14], b[15]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_shape() {
        let u = new_uuid();
        assert_eq!(u.len(), 36);
        assert_eq!(u.matches('-').count(), 4);
        assert_ne!(u, new_uuid());
    }
}
