//! Content addressing for raw evidence.

/// BLAKE3 digest of content, prefixed with the algorithm so future digests
/// can coexist ("blake3:<hex>").
pub fn content_hash(content: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(content).to_hex())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_prefixed() {
        let a = content_hash(b"hello");
        let b = content_hash(b"hello");
        let c = content_hash(b"hello!");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("blake3:"));
        assert_eq!(a.len(), "blake3:".len() + 64);
    }
}
