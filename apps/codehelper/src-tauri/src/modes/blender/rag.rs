use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::path::Path;

const MAX_RETRIEVAL_RESULTS: usize = 10;
const MIN_RELEVANCE_SCORE: f32 = 0.2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RagChunk {
    pub text: String,
    pub signature: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RagContext {
    pub text: String,
    pub signature: String,
    pub url: String,
    pub similarity: f32,
}

#[derive(Debug, Clone)]
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
            Err(error) => return Self::disabled(format!("Failed to load metadata: {error}")),
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
            .filter_map(|(index, similarity)| {
                self.metadata.get(index).map(|chunk| RagContext {
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
        let file = File::open(json_path).map_err(|error| format!("{} ({error})", json_path.display()))?;
        return serde_json::from_reader(file)
            .map_err(|error| format!("Failed parsing {}: {error}", json_path.display()));
    }

    if pickle_path.exists() {
        log::warn!(
            "[BlenderRAG] metadata.json not found; falling back to metadata.pkl at {}",
            pickle_path.display()
        );
        let file =
            File::open(pickle_path).map_err(|error| format!("{} ({error})", pickle_path.display()))?;
        return serde_pickle::from_reader(file, serde_pickle::de::DeOptions::default())
            .map_err(|error| format!("Failed parsing {}: {error}", pickle_path.display()));
    }

    Err(format!(
        "No metadata file found. Tried {} and {}",
        json_path.display(),
        pickle_path.display()
    ))
}

fn keyword_top_k(chunks: &[RagChunk], query: &str, top_k: usize) -> Vec<(usize, f32)> {
    if chunks.is_empty() || query.trim().is_empty() || top_k == 0 {
        return Vec::new();
    }

    let query_terms = tokenize(query);
    if query_terms.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<(usize, f32)> = chunks
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            let combined = format!("{} {}", chunk.signature, chunk.text);
            let chunk_terms = tokenize(&combined);
            let overlap = query_terms.intersection(&chunk_terms).count() as f32;
            let base_score = overlap / query_terms.len() as f32;
            let signature_bonus = if contains_any(&chunk.signature, &query_terms) {
                0.1
            } else {
                0.0
            };

            (index, (base_score + signature_bonus).min(1.0))
        })
        .collect();

    scored.retain(|(_, score)| *score >= MIN_RELEVANCE_SCORE);
    scored.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(top_k.min(scored.len()));
    scored
}

fn tokenize(text: &str) -> HashSet<String> {
    text.to_ascii_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| token.len() > 1 && !is_stopword(token))
        .map(ToString::to_string)
        .collect()
}

fn contains_any(text: &str, terms: &HashSet<String>) -> bool {
    let lowered = text.to_ascii_lowercase();
    terms.iter().any(|term| lowered.contains(term))
}

fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "a"
            | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "can"
            | "do"
            | "for"
            | "from"
            | "how"
            | "i"
            | "in"
            | "into"
            | "is"
            | "it"
            | "me"
            | "my"
            | "of"
            | "on"
            | "or"
            | "should"
            | "show"
            | "that"
            | "the"
            | "tell"
            | "their"
            | "this"
            | "to"
            | "we"
            | "what"
            | "when"
            | "where"
            | "which"
            | "with"
            | "you"
            | "your"
    )
}

#[cfg(test)]
mod tests {
    use super::{RagChunk, RagIndex};
    use tempfile::tempdir;

    #[test]
    fn retrieve_context_returns_top_match() {
        let temp_dir = tempdir().expect("temp dir");
        let db_dir = temp_dir.path().join("simple_db");
        std::fs::create_dir_all(&db_dir).expect("db dir");
        std::fs::write(
            db_dir.join("metadata.json"),
            serde_json::to_string(&vec![
                RagChunk {
                    text: "Use the bevel modifier to soften hard edges".to_string(),
                    signature: "bpy.types.BevelModifier".to_string(),
                    url: "/bpy.types.BevelModifier.html".to_string(),
                },
                RagChunk {
                    text: "Materials control surface appearance".to_string(),
                    signature: "bpy.types.Material".to_string(),
                    url: "/bpy.types.Material.html".to_string(),
                },
            ])
            .expect("metadata json"),
        )
        .expect("write metadata");

        let index = RagIndex::load_from_dir(temp_dir.path());
        let results = index
            .retrieve_context("how do I add a bevel modifier", 1)
            .expect("retrieve");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].signature, "bpy.types.BevelModifier");
    }

    #[test]
    fn disabled_index_returns_empty_results() {
        let index = RagIndex::disabled("missing metadata".to_string());
        let results = index
            .retrieve_context("bevel modifier", 3)
            .expect("retrieve");
        assert!(results.is_empty());
    }
}
