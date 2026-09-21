//! Local ONNX embeddings via fastembed (all-MiniLM-L6-v2, 384 dims).
//!
//! Embedding happens on a blocking thread; the model handle is behind a
//! mutex because `TextEmbedding::embed` needs `&mut self`.

use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

use crate::{EmbeddingError, EmbeddingProvider};

pub const DIMENSIONS: usize = 384;

pub struct FastembedProvider {
    model: Arc<Mutex<TextEmbedding>>,
}

impl FastembedProvider {
    /// Initializes the model, downloading it into `cache_dir` on first use.
    pub fn new(cache_dir: &Path) -> Result<Self, EmbeddingError> {
        let options = TextInitOptions::new(EmbeddingModel::AllMiniLML6V2)
            .with_cache_dir(cache_dir.to_path_buf())
            .with_show_download_progress(false);
        let model =
            TextEmbedding::try_new(options).map_err(|e| EmbeddingError::Init(e.to_string()))?;
        Ok(Self {
            model: Arc::new(Mutex::new(model)),
        })
    }
}

#[async_trait]
impl EmbeddingProvider for FastembedProvider {
    fn model_name(&self) -> &str {
        "all-minilm-l6-v2"
    }

    fn model_version(&self) -> &str {
        // Tracks the fastembed major line producing the vectors.
        "fastembed-6"
    }

    fn dimensions(&self) -> usize {
        DIMENSIONS
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let model = Arc::clone(&self.model);
        let texts = texts.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut guard = model
                .lock()
                .map_err(|_| EmbeddingError::Embed("embedding model mutex poisoned".into()))?;
            guard
                .embed(&texts, None)
                .map_err(|e| EmbeddingError::Embed(e.to_string()))
        })
        .await
        .map_err(|e| EmbeddingError::Embed(format!("embedding task panicked: {e}")))?
    }
}
