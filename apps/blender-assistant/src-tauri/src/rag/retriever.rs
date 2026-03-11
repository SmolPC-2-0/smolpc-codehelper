use crate::rag::types::RagChunk;
use std::collections::HashSet;

const MIN_RELEVANCE_SCORE: f32 = 0.2;

pub fn keyword_top_k(chunks: &[RagChunk], query: &str, top_k: usize) -> Vec<(usize, f32)> {
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
        .map(|(idx, chunk)| {
            let combined = format!("{} {}", chunk.signature, chunk.text);
            let chunk_terms = tokenize(&combined);
            let overlap = query_terms.intersection(&chunk_terms).count() as f32;

            // Normalized overlap score with a small signature bonus.
            let base_score = overlap / query_terms.len() as f32;
            let signature_bonus = if contains_any(&chunk.signature, &query_terms) {
                0.1
            } else {
                0.0
            };

            (idx, (base_score + signature_bonus).min(1.0))
        })
        .collect();

    scored.retain(|(_, score)| *score >= MIN_RELEVANCE_SCORE);
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k.min(scored.len()));
    scored
}

fn tokenize(text: &str) -> HashSet<String> {
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_alphanumeric())
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
            | "my"
            | "you"
            | "your"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_top_match_for_overlapping_terms() {
        let chunks = vec![
            RagChunk {
                text: "Use bevel modifier to smooth hard edges".to_string(),
                signature: "bpy.types.BevelModifier".to_string(),
                url: "/bpy.types.BevelModifier.html".to_string(),
            },
            RagChunk {
                text: "Material settings control surface appearance".to_string(),
                signature: "bpy.types.Material".to_string(),
                url: "/bpy.types.Material.html".to_string(),
            },
        ];

        let results = keyword_top_k(&chunks, "how do I add a bevel modifier", 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0);
    }

    #[test]
    fn filters_out_stopword_only_overlap() {
        let chunks = vec![RagChunk {
            text: "A guide for how to do this in Blender".to_string(),
            signature: "generic guide".to_string(),
            url: "/guide".to_string(),
        }];

        let results = keyword_top_k(
            &chunks,
            "how do I do this for my scene and animation",
            1,
        );
        assert!(results.is_empty());
    }
}
