use crate::rag::retriever::keyword_top_k;
use crate::rag::types::{RagChunk, RagContext};
use std::fs::File;
use std::path::Path;

const MAX_RETRIEVAL_RESULTS: usize = 10;

pub struct RagIndex {
    metadata: Vec<RagChunk>,
    loaded: bool,
    load_error: Option<String>,
}

impl RagIndex {
    pub fn load_from_dir(rag_dir: &Path) -> Self {
        let db_dir = rag_dir.join("simple_db");
        let metadata_json_path = db_dir.join("metadata.json");
        let metadata_pickle_path = db_dir.join("metadata.pkl");

        let metadata = match load_metadata(&metadata_json_path, &metadata_pickle_path) {
            Ok(value) if !value.is_empty() => value,
            Ok(_) => return Self::disabled("Metadata file is empty".to_string()),
            Err(e) => return Self::disabled(format!("Failed to load metadata: {}", e)),
        };

        Self {
            metadata,
            loaded: true,
            load_error: None,
        }
    }

    pub fn disabled(reason: String) -> Self {
        Self {
            metadata: Vec::new(),
            loaded: false,
            load_error: Some(reason),
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn document_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn load_error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }

    pub fn retrieve_context(&self, query: &str, n_results: usize) -> Result<Vec<RagContext>, String> {
        if !self.loaded {
            return Ok(Vec::new());
        }

        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let scored = keyword_top_k(
            &self.metadata,
            trimmed,
            n_results.clamp(1, MAX_RETRIEVAL_RESULTS),
        );

        Ok(scored
            .into_iter()
            .filter_map(|(idx, similarity)| {
                self.metadata.get(idx).map(|chunk| RagContext {
                    text: chunk.text.clone(),
                    signature: chunk.signature.clone(),
                    url: chunk.url.clone(),
                    similarity,
                })
            })
            .collect())
    }
}

fn load_metadata(json_path: &Path, pickle_path: &Path) -> Result<Vec<RagChunk>, String> {
    if json_path.exists() {
        let file = File::open(json_path).map_err(|e| format!("{} ({})", json_path.display(), e))?;
        return serde_json::from_reader(file)
            .map_err(|e| format!("Failed parsing {}: {}", json_path.display(), e));
    }

    if pickle_path.exists() {
        log::warn!(
            "[RAG] metadata.json not found; falling back to metadata.pkl at {}",
            pickle_path.display()
        );
        let file =
            File::open(pickle_path).map_err(|e| format!("{} ({})", pickle_path.display(), e))?;
        return serde_pickle::from_reader(file, serde_pickle::de::DeOptions::default())
            .map_err(|e| format!("Failed parsing {}: {}", pickle_path.display(), e));
    }

    Err(format!(
        "No metadata file found. Tried {} and {}",
        json_path.display(),
        pickle_path.display()
    ))
}
