//! Hash-based embedding provider for tests and development.
//!
//! No semantic quality — texts sharing literal tokens land near each other,
//! which is exactly what deterministic retrieval tests need. Never use in
//! production; the model name makes accidental mixing with real embeddings
//! impossible.

use async_trait::async_trait;

use crate::{EmbeddingError, EmbeddingProvider};

pub const DIMENSIONS: usize = 384;

pub struct DeterministicProvider;

impl DeterministicProvider {
    pub fn new() -> Self {
        Self
    }

    fn embed_one(text: &str) -> Vec<f32> {
        let mut v = vec![0.0f32; DIMENSIONS];
        for token in text
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
        {
            let bucket = (fnv1a(token.as_bytes()) as usize) % DIMENSIONS;
            v[bucket] += 1.0;
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut v {
                *x /= norm;
            }
        }
        v
    }
}

impl Default for DeterministicProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// FNV-1a: stable across platforms and Rust releases, unlike DefaultHasher.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[async_trait]
impl EmbeddingProvider for DeterministicProvider {
    fn model_name(&self) -> &str {
        "deterministic-hash"
    }

    fn model_version(&self) -> &str {
        "1"
    }

    fn dimensions(&self) -> usize {
        DIMENSIONS
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|t| Self::embed_one(t)).collect())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    #[tokio::test]
    async fn shared_tokens_score_higher() {
        let p = DeterministicProvider::new();
        let vs = p
            .embed(&[
                "authentication architecture middleware".to_string(),
                "the authentication flow uses middleware".to_string(),
                "grocery shopping list bananas".to_string(),
            ])
            .await
            .unwrap();
        assert_eq!(vs[0].len(), DIMENSIONS);
        assert!(cosine(&vs[0], &vs[1]) > cosine(&vs[0], &vs[2]));
    }

    #[tokio::test]
    async fn embedding_is_deterministic() {
        let p = DeterministicProvider::new();
        let a = p.embed(&["same input".to_string()]).await.unwrap();
        let b = p.embed(&["same input".to_string()]).await.unwrap();
        assert_eq!(a, b);
    }
}
